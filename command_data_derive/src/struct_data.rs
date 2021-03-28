use std::iter::FromIterator;

use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro_error::*;
use quote::{quote, quote_spanned};
use syn::{Attribute, Fields, Ident, Index, Lifetime, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta, Path, Type};
use syn::spanned::Spanned;

use crate::utils::*;

pub fn struct_impl(ty: &Ident, fields: Fields, attributes: &[Attribute]) -> TokenStream2 {
    let strukt = Struct::from_fields(fields, attributes);
    let (command_data_impl, command_type) = command_data_impl(strukt.command_type.as_ref());
    let from_options_body = strukt.impl_from_options(ty, &Path::from(ty.clone()), &command_type, 0);
    let data_options = strukt.data_options();

    let tokens = quote! {
        #command_data_impl for #ty {
            // all structs are built from a Vec<ValueOption>
            type Options = ::std::vec::Vec<::discorsd::model::interaction::ValueOption>;

            fn from_options(
                options: Self::Options,
            ) -> ::std::result::Result<Self, ::discorsd::errors::CommandParseError> {
                #from_options_body
            }

            // all structs are DataOptions
            type VecArg = ::discorsd::commands::DataOption;

            fn make_args(command: &#command_type) -> Vec<Self::VecArg> {
                #data_options
            }
        }
    };
    tokens
}

#[derive(Debug)]
pub struct Field {
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
handle_attribute!(self Field =>
    " (without a value)": Meta::Path(path), path =>
        /// Uses this field's `Default` implementation if this field is missing.
        ["default" => self.default = Some(syn::parse_str("::std::default::Default::default").unwrap())]
        // todo is this necessary?
        /// Make this field required (note: fields are required by default, unless they are an `Option`.
        ["required" => self.default = None]
        /// Name this vararg field One, Two, Three, etc.
        ["ordinals" => self.vararg.names = VarargNames::Ordinals]
        /// Name this vararg field <vararg>1, <vararg>2, where <vararg> is the key on the `vararg` option.
        ["counts" => self.vararg.names = VarargNames::Count],

    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// The description of this command option.
        ["desc" => self.desc = Some(str)]
        /// Use this path to provide the default if this field is missing.
        /// Must be callable as `fn() -> T`
        ["default" => self.default = Some(str.parse()?)]
        /// Marks this field as a vararg argument to the command, with the name `{str}`.
        /// See also `ordinals`, `counts`, `va_count`, and `va_names`
        ["vararg" => self.vararg.root = Some(str)]
        /// How to filter the choices, if `choices` is true.
        ///
        /// Must be a function callable as
        /// `fn<C: SlashCommand>(command: &C, choice: &CommandChoice<&'static str>) -> bool`
        /// if the type for this data is not set, or as
        /// `fn(command: &C, choice: &CommandChoice<&'static str) -> bool`
        /// where `C` is the right hand side of `#[command(type = ...)]` on the struct if it is.
        ["retain" => self.retain = Some(str.parse()?)]
        /// Function to determine if this field is required, must be callable as
        /// `fn<C: SlashCommand>(command: &C) -> bool`, where the generic is not necessary if the
        /// struct's type is specified (`#[command(type = "MyCommand")]`.
        ["required" => self.required = Some(str.parse()?)]
        /// `fn<C: SlashCommand>(command: &C) -> usize` to pick how many vararg options to display.
        ["va_count" => self.vararg.num = VarargNum::Function(str.parse()?)]
        /// How to name the vararg options.
        ["va_names" => self.vararg.names = VarargNames::Function(str.parse()?)]
        /// What to rename this field as in the Command.
        ["rename" => {
            if let FieldIdent::Named(named) = &mut self.name {
                named.rename = Some(str);
            }
        }],

    " = {int}": Meta::NameValue(MetaNameValue { path, lit: Lit::Int(int), .. }), path =>
        /// The number of vararg options to show.
        ["va_count" => self.vararg.num = VarargNum::Count(int.base10_parse()?)]
);

#[derive(Debug)]
pub enum FieldIdent {
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
pub struct NamedField {
    ident: Ident,
    rename: Option<LitStr>,
}

#[derive(Debug)]
pub struct UnnamedField {
    index: Index,
    // todo presumably rename
}

#[derive(Debug, Default)]
struct Vararg {
    /// the root of the vararg parameter, for example `player` for player1, player2, player3, ...
    root: Option<LitStr>,
    /// `fn<C: SlashCommand>(command: &C) -> usize` to pick how many vararg options to display
    num: VarargNum,
    /// how to name the vararg options
    names: VarargNames,
}

impl Vararg {
    const fn is_some(&self) -> bool {
        self.root.is_some()
    }
}

#[derive(Debug)]
enum VarargNum {
    Count(usize),
    Function(Path),
}

impl VarargNum {
    fn take_fn(&self) -> TokenStream2 {
        match self {
            VarargNum::Count(n) => {
                quote! { (|_| n) }
            }
            VarargNum::Function(path) => {
                quote! { path }
            }
        }
    }
}

impl Default for VarargNum {
    fn default() -> Self {
        Self::Count(0)
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

#[allow(clippy::fallible_impl_from)]
impl From<syn::Field> for Field {
    fn from(field: syn::Field) -> Self {
        let attrs = field.attrs;
        let mut field = Self {
            name: FieldIdent::Named(NamedField {
                ident: field.ident.expect("named fields"),
                rename: None,
            }),
            default: None,
            ty: field.ty,
            vararg: Default::default(),
            retain: None,
            desc: None,
            required: None,
        };

        if field.ty.generic_type_of("Option").is_some() {
            field.default = Some(syn::parse_str("::std::default::Default::default").unwrap());
        }
        for attr in attrs {
            if !attr.path.is_ident("command") { continue; }

            field.handle_attribute(&attr);
        }

        field
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<(usize, syn::Field)> for Field {
    fn from((i, field): (usize, syn::Field)) -> Self {
        let attrs = field.attrs;
        let mut field = Self {
            name: FieldIdent::Unnamed(UnnamedField {
                index: Index::from(i)
            }),
            default: None,
            ty: field.ty,
            vararg: Default::default(),
            retain: None,
            desc: None,
            required: None,
        };

        if field.ty.generic_type_of("Option").is_some() {
            field.default = Some(syn::parse_str("::std::default::Default::default").unwrap());
        }
        for attr in attrs {
            if !attr.path.is_ident("command") { continue; }

            field.handle_attribute(&attr);
        }

        field
    }
}

#[derive(Debug)]
pub struct Struct {
    fields: Vec<Field>,
    /// settable with `#[command(type = MyCommand)]` on a struct
    command_type: Option<Type>,
}
handle_attribute!(self Struct =>
    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// Specify the type of the `SlashCommand` that this is data for. Useful for annotations that
        /// can make decisions at runtime by taking functions callable as `fn(CommandType) -> SomeType`.
        ["type" => self.command_type = Some(str.parse()?)]
);

impl Struct {
    const UNIT: Self = Self { fields: Vec::new(), command_type: None };

    pub fn from_fields(fields: Fields, attributes: &[Attribute]) -> Self {
        let mut strukt = match fields {
            Fields::Named(fields) => fields.named
                .into_iter()
                .map(Field::from)
                .collect(),
            Fields::Unnamed(fields) => fields.unnamed
                .into_iter()
                .enumerate()
                .map(Field::from)
                .collect(),
            Fields::Unit => Struct::UNIT,
        };
        for attr in attributes {
            if !attr.path.is_ident("command") { continue; }
            strukt.handle_attribute(attr);
        }
        strukt
    }

    pub fn impl_from_options(&self, return_type: &Ident, return_ctor: &Path, command_ty: &TokenStream2, n: usize) -> TokenStream2 {
        let num_fields = self.fields.len();
        let builder_struct = self.builder_struct(return_type, return_ctor);
        let field_eq = self.field_eq();
        let fields_array = self.fields_array();
        let fields_match = self.match_branches(command_ty);
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
                                _ => return Err(CommandParseError::BadOrder(option.name, idx, 0..#num_fields))
                            }
                            i = idx;
                            if !vararg_matches { i += 1 }
                            continue #label;
                        }
                    }
                    // the option MUST match one of the fields, if it didn't that's an error
                    return Err(CommandParseError::UnknownOption(UnknownOption {
                        name: option.name,
                        options: &FIELDS,
                    }))
                }
            }
        };

        quote! {
            use ::discorsd::errors::*;

            // declares the type and makes a mutable instance named `builder`
            #builder_struct

            #build_struct

            // Ok(builder.build().map_err(|e| (e, id))?)
            builder.build()
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

    fn match_branches(&self, command_ty: &TokenStream2) -> TokenStream2 {
        let branches = self.fields.iter().enumerate().map(|(i, f)| {
            let name = f.name.builder_ident();
            if f.vararg.is_some() {
                let vararg_type = &f.ty.without_generics();
                let generic_type = &f.ty.generic_type();
                quote! {
                    #i => {
                        // `id` is declared in the impl of TryFrom
                        let value: #generic_type = <#generic_type as ::discorsd::model::commands::CommandData<#command_ty>>
                                                    ::from_options(option)?;
                        // `builder` is made in `builder_struct`,
                        let collection = builder.#name.get_or_insert(#vararg_type::default());
                        collection.extend(::std::iter::once(value));
                    }
                }
            } else {
                let ty = &f.ty;
                quote! {
                    #i => builder.#name = Some(
                        <#ty as ::discorsd::model::commands::CommandData<#command_ty>>::from_options(option)?
                    )
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
                quote! { ok_or_else(|| ::discorsd::errors::CommandParseError::MissingOption(#name.to_string()))? }
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
                fn build(self) -> Result<#return_type, ::discorsd::errors::CommandParseError> {
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

    pub fn data_options(&self) -> TokenStream2 {
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
        let retain = if let Some(path) = &f.retain {
            quote_spanned! { path.span() =>
                    choices.retain(|choice| #path(command, choice));
                }
        } else {
            TokenStream2::new()
        };
        quote_spanned! { f.name.span() =>
            {
                #[allow(unused_mut)]
                let mut choices = #ty::make_choices(command);
                #retain
                if choices.is_empty() {
                    #[allow(unused_mut)]
                    let mut option = ::discorsd::commands::CommandDataOption::new(#name, #desc);
                    #required
                    #ty::option_ctor(option)
                } else {
                    let mut option = ::discorsd::commands::CommandDataOption::new_str(#name, #desc);
                    option = option.choices(choices);
                    #required
                    ::discorsd::commands::DataOption::String(option)
                }
            }
        }
    }

    fn vararg_option(f: &Field) -> TokenStream2 {
        let root = f.vararg.root.as_ref().unwrap();
        let names = f.vararg.names.names(root);
        let take = f.vararg.num.take_fn();
        let descriptions = VarargNames::Count.names(root);
        quote! {
            // `command` is in scope from `CommandArgs` impl
            #names
                .take(#take(command))
                .zip(#descriptions)
                .map
        }
    }
}

impl FromIterator<Field> for Struct {
    fn from_iter<I: IntoIterator<Item=Field>>(iter: I) -> Self {
        let fields: Vec<Field> = iter.into_iter().collect();
        Self { fields, command_type: None }
    }
}
