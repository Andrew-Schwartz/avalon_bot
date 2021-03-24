use std::convert::TryFrom;
use std::iter::FromIterator;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use syn::{Attribute, Fields, Ident, Index, Lifetime, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta, Path, Type};
use syn::spanned::Spanned;

use crate::utils::TypeExt;

pub fn struct_impl(ty: &Ident, fields: Fields, attributes: &[Attribute]) -> TokenStream2 {
    let strukt = match Struct::from_fields(fields, attributes) {
        Ok(fields) => fields,
        Err(e) => return e.into_compile_error(),
    };
    let try_from_body = strukt.try_from_body(ty, &Path::from(ty.clone()), 0);
    let (args_impl_statement, command_type) = strukt.args_impl_statement();
    let data_options = strukt.data_options();

    let tokens = quote! {
        impl ::std::convert::TryFrom<ApplicationCommandInteractionData> for #ty {
            type Error = (discorsd::errors::CommandParseError, discorsd::model::ids::CommandId);

            fn try_from(data: ApplicationCommandInteractionData) -> ::std::result::Result<Self, Self::Error> {
                let id = data.id;
                let options = data.options;

                #try_from_body
            }
        }

        #args_impl_statement for #ty {
            fn args(command: &#command_type) -> discorsd::model::interaction::TopLevelOption {
                discorsd::model::interaction::TopLevelOption::Data(#data_options)
            }
        }
    };
    tokens
}

#[derive(Debug)]
struct Field {
    name: FieldIdent,
    ty: Type,
    /// for example, Default::default or Instant::now
    default: Option<Path>,
    /// function to determine if this field is required, must be callable as
    /// `fn<C: SlashCommand>(command: &C) -> bool`, where the generic is not necessary if the
    /// struct's type is specified (`#[command(type = "MyCommand")]`
    required: Option<Path>,
    /// see [Vararg] for details
    vararg: Vararg,
    /// whether to generate `.choices(Self::choices())` for this field's `DataOption`
    choices: bool,
    /// how to filter the choices, if `choices` is true
    ///
    /// must be a function callable as
    /// `fn<C: SlashCommand>(command: &C, choice: &CommandChoice<&'static str>) -> bool`
    /// if the type for this data is not set, or as
    /// `fn(command: &C, choice: &CommandChoice<&'static str) -> bool`
    /// where `C` is the right hand side of `#[command(type = ...)]` on the struct if
    retain: Option<Path>,
    /// The description of this `DataOption`
    desc: Option<LitStr>,
}

#[derive(Debug)]
enum FieldIdent {
    Named(NamedField),
    Unnamed(UnnamedField),
}

impl FieldIdent {
    fn builder_ident(&self) -> Ident {
        match self {
            Self::Named(named) => Ident::new(&named.ident.to_string(), named.ident.span()),
            Self::Unnamed(UnnamedField { index }) => Ident::new(&format!("_{}", index.index), index.span)
        }
    }

    fn span(&self) -> Span {
        match self {
            Self::Named(named) => named.ident.span(),
            Self::Unnamed(unnamed) => unnamed.index.span,
        }
    }
}

#[derive(Debug)]
struct NamedField {
    ident: Ident,
    rename: Option<LitStr>,
}

#[derive(Debug)]
struct UnnamedField {
    index: Index,
}

#[derive(Debug, Default)]
struct Vararg {
    /// the root of the vararg parameter, for example `player` for player1, player2, player3, ...
    root: Option<LitStr>,
    /// `fn<C: SlashCommand>(command: &C) -> usize` to pick how many vararg options to display
    num: Option<Path>,
    /// how to name the vararg options
    names: VarargNames,
}

impl Vararg {
    const fn is_some(&self) -> bool {
        self.root.is_some()
    }
}

#[derive(Debug)]
enum VarargNames {
    /// if root is `player`, names will be `player1`, `player2`, etc
    Count,
    /// names will be `First`, `Second`, `Third`, etc (up to 20)
    Ordinals,
    /// names given by this function, callable as `fn<C: Into<Cow<'static, str>>(n: usize) -> C`
    Function(Path),
}

impl Default for VarargNames {
    fn default() -> Self {
        Self::Count
    }
}

impl VarargNames {
    fn names(&self, root: &LitStr) -> TokenStream2 {
        match self {
            VarargNames::Count => {
                let names = (1..).map(|i| format!("{}{}", root.value(), i));
                quote! {
                    [#(#names),*].iter()
                }
            }
            VarargNames::Ordinals => quote! {
                [
                    "First", "Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh", "Eighth",
                    "Ninth", "Tenth", "Eleventh", "Twelfth", "Thirteenth", "Fourteenth",
                    "Fifteenth", "Sixteenth", "Seventeenth", "Eighteenth", "Nineteenth", "Twentieth"
                ].iter()
            },
            VarargNames::Function(fun) => {
                let names = (1..).map(|i| quote! { #fun(#i) });
                quote! {
                    [#(#names),*].iter()
                }
            }
        }
    }
}

impl Field {
    fn arg_name(&self) -> String {
        match &self.name {
            FieldIdent::Named(named) => named.rename.as_ref()
                .map(LitStr::value)
                .or_else(|| self.vararg.root.as_ref().map(LitStr::value))
                .unwrap_or_else(|| named.ident.to_string()),
            FieldIdent::Unnamed(unnamed) => self.vararg.root.as_ref()
                .map_or_else(|| unnamed.index.index.to_string(), LitStr::value),
        }
    }
}

impl TryFrom<syn::Field> for Field {
    type Error = syn::Error;

    fn try_from(field: syn::Field) -> Result<Self, Self::Error> {
        let attrs = field.attrs;
        let mut field = Self {
            name: FieldIdent::Named(NamedField {
                ident: field.ident.expect("named fields"),
                rename: None,
            }),
            default: None,
            ty: field.ty,
            vararg: Default::default(),
            choices: false,
            retain: None,
            desc: None,
            required: None,
        };

        if field.ty.generic_type_of("Option").is_some() {
            field.default = Some(syn::parse_str("Default::default")?);
        }
        for attr in attrs {
            if !attr.path.is_ident("command") { continue; }

            field.handle_attribute(&attr)?;
        }

        Ok(field)
    }
}

impl TryFrom<(usize, syn::Field)> for Field {
    type Error = syn::Error;

    fn try_from((i, field): (usize, syn::Field)) -> Result<Self, Self::Error> {
        let attrs = field.attrs;
        let mut field = Self {
            name: FieldIdent::Unnamed(UnnamedField {
                index: Index::from(i)
            }),
            default: None,
            ty: field.ty,
            vararg: Default::default(),
            choices: false,
            retain: None,
            desc: None,
            required: None,
        };

        if field.ty.generic_type_of("Option").is_some() {
            field.default = Some(syn::parse_str("Default::default")?);
        }
        for attr in attrs {
            if !attr.path.is_ident("command") { continue; }

            field.handle_attribute(&attr)?;
        }

        Ok(field)
    }
}

impl Field {
    fn handle_attribute(&mut self, attr: &Attribute) -> Result<(), syn::Error> {
        let meta = attr.parse_meta().expect("failed to parse meta");
        match meta {
            Meta::List(MetaList { nested, .. }) => {
                for n in nested {
                    match n {
                        NestedMeta::Meta(
                            Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. })
                        ) => {
                            if path.is_ident("rename") {
                                if let FieldIdent::Named(named) = &mut self.name {
                                    named.rename = Some(str);
                                }
                            } else if path.is_ident("default") {
                                // has to be parse_str so that the types don't get messed up
                                self.default = Some(str.parse()?);
                                // self.default = Some(syn::parse_str(&str.value())?);
                            } else if path.is_ident("vararg") {
                                self.vararg.root = Some(str);
                            } else if path.is_ident("desc") {
                                self.desc = Some(str);
                            } else if path.is_ident("retain") {
                                self.retain = Some(str.parse()?);
                            } else if path.is_ident("required") {
                                self.required = Some(str.parse()?);
                            } else if path.is_ident("va_count") {
                                self.vararg.num = Some(str.parse()?);
                            } else if path.is_ident("va_names") {
                                self.vararg.names = VarargNames::Function(str.parse()?);
                            }
                        }
                        NestedMeta::Meta(Meta::Path(path)) => {
                            if path.is_ident("default") {
                                self.default = Some(syn::parse_str("Default::default")?);
                            } else if path.is_ident("choices") {
                                self.choices = true;
                            } else if path.is_ident("required") {
                                self.default = None;
                            } else if path.is_ident("ordinals") {
                                self.vararg.names = VarargNames::Ordinals;
                            } else if path.is_ident("count") {
                                self.vararg.names = VarargNames::Count;
                            }
                        }
                        // todo just impl whatever it is on [Type; N]
                        // NestedMeta::Meta(
                        //     Meta::NameValue(MetaNameValue{ path, lit: Lit::Int(int), .. })
                        // ) => {
                        //     if path.is_ident("va_count") {
                        //         int.
                        //     }
                        // }
                        _ => eprintln!("(field) n = {:?}", n),
                    }
                }
            }
            _ => eprintln!("(field) meta = {:?}", meta),
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct Struct {
    fields: Vec<Field>,
    /// settable with `#[command(type = MyCommand)]` on a struct
    command: Option<Ident>,
}

impl Struct {
    const UNIT: Self = Self { fields: Vec::new(), command: None };

    pub fn from_fields(fields: Fields, attributes: &[Attribute]) -> Result<Self, syn::Error> {
        let mut strukt = match fields {
            Fields::Named(fields) => fields.named
                .into_iter()
                .map(Field::try_from)
                .collect(),
            Fields::Unnamed(fields) => fields.unnamed
                .into_iter()
                .enumerate()
                .map(Field::try_from)
                .collect(),
            Fields::Unit => Ok(Struct::UNIT),
        }?;
        for attr in attributes {
            if !attr.path.is_ident("command") { continue; }
            strukt.handle_attribute(attr)?;
        }
        Ok(strukt)
    }

    fn handle_attribute(&mut self, attr: &Attribute) -> Result<(), syn::Error> {
        let meta = attr.parse_meta().expect("failed to parse meta");
        match meta {
            Meta::List(MetaList { nested, .. }) => {
                for n in nested {
                    match n {
                        NestedMeta::Meta(
                            Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. })
                        ) => {
                            if path.is_ident("type") {
                                self.command = Some(str.parse()?);
                            }
                        }
                        _ => eprint!("(struct) n = {:?}", n),
                    }
                }
            }
            _ => eprintln!("(struct) meta = {:?}", meta),
        }
        Ok(())
    }

    pub fn try_from_body(&self, return_type: &Ident, return_ctor: &Path, n: usize) -> TokenStream2 {
        let num_fields = self.fields.len();
        let builder_struct = self.builder_struct(return_type, return_ctor);
        let field_eq = self.field_eq();
        let fields_array = self.fields_array();
        let fields_match = self.match_branches();
        let varargs_array = self.varargs_array();
        let label = Lifetime {
            apostrophe: Span::call_site(),
            ident: Ident::new(&format!("opt{}", n), Span::call_site()),
        };

        let build_struct = if self.fields.is_empty() {
            // if there are no fields (ie, is Unit struct), don't have to parse any options
            TokenStream2::new()
        } else {
            quote! {
                const FIELDS: [&'static str; #num_fields] = #fields_array;
                const VARARGS: [Option<&'static str>; #num_fields] = #varargs_array;

                let mut i = 0;
                #label: for option in options {
                    for idx in i..#num_fields {
                        // let idx = idx + i;
                        // eprintln!("idx = {}, i = {}", idx, i);
                        let vararg_matches = matches!(
                            VARARGS[idx],
                            Some(vararg) if matches!(
                                option.name.strip_prefix(vararg),
                                Some(num) if num.parse::<usize>().is_ok()
                            )
                        );
                        // eprintln!("idx = {}, fields = {:?}", idx, FIELDS);
                        if #field_eq(idx) || vararg_matches {
                            #[allow(clippy::used_underscore_binding)]
                            match idx {
                                #fields_match
                                // there were too many args
                                _ => return Err((CommandParseError::BadOrder(option.name, idx, 0..#num_fields), id))
                            }
                            i = idx;
                            if !vararg_matches { i += 1 }
                            continue #label;
                        }
                    }
                    // the option MUST match one of the fields, if it didn't that's an error
                    return Err((CommandParseError::UnknownOption(UnknownOption {
                        name: option.name,
                        options: &FIELDS,
                    }), id))
                }
            }
        };

        quote! {
            use discorsd::model::commands::CommandOptionInto;
            use discorsd::errors::*;

            // declares the type and makes a mutable instance named `builder`
            #builder_struct

            #build_struct

            Ok(builder.build().map_err(|e| (e, id))?)
        }
    }

    fn field_eq(&self) -> TokenStream2 {
        match &self.fields.first().map(|f| &f.name) {
            // named fields, compare name to `opt.name`
            Some(FieldIdent::Named(_)) => quote! { (|idx: usize| FIELDS[idx] == option.name) },
            // unnamed fields, just select in order
            Some(FieldIdent::Unnamed(_)) => quote! { (|idx: usize| true) },
            // Unit struct, not fields so not reachable
            None => quote! { (|idx: usize| unreachable!()) }
        }
    }

    fn fields_array(&self) -> TokenStream2 {
        let fields = self.fields.iter().map(Field::arg_name);
        quote! { [#(#fields),*] }
    }

    fn match_branches(&self) -> TokenStream2 {
        let branches = self.fields.iter().enumerate().map(|(i, f)| {
            let name = f.name.builder_ident();
            if f.vararg.is_some() {
                let vararg_type = &f.ty.without_generics();
                let generic_type = &f.ty.generic_type();
                quote! {
                    #i => {
                        // `id` is declared in the impl of TryFrom
                        let value: #generic_type = option.try_into().map_err(|e| (e, id))?;
                        // `builder` is made in `builder_struct`,
                        let collection = builder.#name.get_or_insert(#vararg_type::default());
                        collection.extend(::std::iter::once(value));
                    }
                }
            } else {
                quote! {
                    #i => builder.#name = Some(option.try_into().map_err(|e| (e, id))?)
                }
            }
        });
        quote! {
            #(#branches,)*
        }
    }

    fn builder_struct(&self, return_type: &Ident, return_ctor: &Path) -> TokenStream2 {
        let builder_prefix: String = return_ctor.segments.iter()
            .map(|seq| seq.ident.to_string())
            .collect();
        let name = Ident::new(&format!("{}Builder", builder_prefix), Span::call_site());
        let fields = self.fields.iter().map(|f| {
            let ident = &f.name.builder_ident();
            let ty = &f.ty;
            quote_spanned! { f.name.span() => #ident: Option<#ty> }
        });
        let builder = self.fields.iter().map(|f| {
            let builder_ident = &f.name.builder_ident();
            let self_ident = match &f.name {
                FieldIdent::Named(named) => {
                    let ident = &named.ident;
                    quote_spanned! { named.ident.span() => #ident }
                }
                FieldIdent::Unnamed(unnamed) => {
                    let index = &unnamed.index;
                    quote_spanned! { unnamed.index.span => #index }
                }
            };
            let name = self_ident.to_string();
            let opt_handler = if let Some(path) = &f.default {
                quote_spanned! { path.span() => unwrap_or_else(#path) }
            } else {
                quote! { ok_or_else(|| discorsd::errors::CommandParseError::MissingOption(#name.to_string()))? }
            };
            quote_spanned! {
                f.name.span() => #self_ident: self.#builder_ident.#opt_handler
            }
        });
        quote! {
            #[derive(Default)]
            struct #name {
                #(#fields),*
            }

            impl #name {
                fn build(self) -> Result<#return_type, discorsd::errors::CommandParseError> {
                    #[allow(clippy::used_underscore_binding)]
                    Ok(#return_ctor {
                        #(#builder),*
                    })
                }
            }

            let mut builder = #name::default();
        }
    }

    fn varargs_array(&self) -> TokenStream2 {
        let vararg_names = self.fields.iter().map(|f| {
            if let Some(vararg) = &f.vararg.root {
                quote_spanned! { f.name.span() => Some(#vararg) }
            } else {
                quote_spanned! { f.name.span() => None }
            }
        });
        quote! { [#(#vararg_names),*] }
    }

    fn args_impl_statement(&self) -> (TokenStream2, TokenStream2) {
        match &self.command {
            None => (
                quote! {
                    impl<C: discorsd::commands::SlashCommand> discorsd::model::commands::CommandArgs<C>
                }, quote! {
                    C
                }),
            Some(ident) => (
                quote_spanned! { ident.span() =>
                    impl discorsd::model::commands::CommandArgs<#ident>
                }, quote_spanned! { ident.span() =>
                    #ident
                }),
        }
    }

    fn data_options(&self) -> TokenStream2 {
        let options = self.fields.iter().map(|f|
            // if f.vararg.is_some() {
            //     Struct::vararg_option(f)
            // } else {
            //     Struct::single_option(f)
            // }
            Struct::single_option(f)
        );
        quote! { vec![#(#options),*] }
    }

    fn single_option(f: &Field) -> TokenStream2 {
        let name = f.arg_name();
        let desc = f.desc.as_ref()
            .map_or_else(|| f.arg_name(), LitStr::value);
        let required = if f.default.is_none() {
            quote! {
                option = option.required();
            }
        } else if let Some(required) = &f.required {
            quote! {
                if #required(command) {
                    option = option.required();
                }
            }
        } else {
            TokenStream2::new()
        };
        // todo is there some way to get the type out in a nice way? (the `generic_type` below is sketch)
        let ty = f.ty.generic_type().unwrap_or(&f.ty);
        if f.choices {
            let retain = if let Some(path) = &f.retain {
                quote_spanned! { path.span() =>
                    choices.retain(|choice| #path(command, choice));
                }
            } else {
                TokenStream2::new()
            };
            quote_spanned! { f.name.span() =>
                discorsd::commands::DataOption::String(
                    {
                        use discorsd::model::commands::OptionChoices;
                        #[allow(unused_mut)]
                        let mut choices = #ty::choices();
                        #retain
                        #[allow(unused_mut)]
                        let mut option = discorsd::commands::CommandDataOption::new_str(#name, #desc).choices(choices);
                        #required
                        option
                    }
                )
            }
        } else {
            quote_spanned! { f.name.span() =>
                {
                    use discorsd::model::commands::OptionCtor;
                    #[allow(unused_mut)]
                    let mut option = discorsd::commands::CommandDataOption::new(#name, #desc);
                    #required
                    #ty::option_ctor(option)
                }
            }
        }
    }

    fn vararg_option(_f: &Field) -> TokenStream2 {
        todo!()
        // let root = f.vararg.root.as_ref().unwrap();
        // let names = f.vararg.names.names(root);
        // let take = f.vararg.num.as_ref().unwrap();
        // let descriptions = VarargNames::Count.names(root);
        // quote! {
        //     // `command` is in scope from `CommandArgs` impl
        //     #names
        //         .take(#take(command))
        //         .zip(#descriptions)
        //         .map
        // }
    }
}

impl FromIterator<Field> for Struct {
    fn from_iter<I: IntoIterator<Item=Field>>(iter: I) -> Self {
        let fields: Vec<Field> = iter.into_iter().collect();
        Self { fields, command: None }
    }
}
