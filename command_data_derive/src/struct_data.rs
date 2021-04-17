use std::iter::FromIterator;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use syn::{Attribute, Fields, Ident, Index, LitStr, Path, Type};
use syn::spanned::Spanned;

use crate::utils::*;

pub fn struct_impl(ty: &Ident, fields: Fields, attributes: &[Attribute]) -> TokenStream2 {
    let strukt = Struct::from_fields(fields, attributes);
    let (command_data_impl, command_type) = command_data_impl(strukt.command_type.as_ref());
    let from_options_body = strukt.impl_from_options(
        ty,
        &Path::from(ty.clone()),
        &command_type,
    );
    let data_options = strukt.data_options(&command_type);

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
    pub name: FieldIdent,
    pub ty: Type,
    /// for example, Default::default or Instant::now
    pub default: Option<Path>,
    /// function to determine if this field is required, must be callable as
    /// `fn<C: SlashCommand>(command: &C) -> bool`, where the generic is not necessary if the
    /// struct's type is specified (`#[command(type = "MyCommand")]`
    pub required: Option<Path>,
    /// see [Vararg](Vararg) for details
    pub vararg: Vararg,
    /// how to filter the choices, if `choices` is true
    ///
    /// must be a function callable as
    /// `fn<C: SlashCommand>(command: &C, choice: &CommandChoice<&'static str>) -> bool`
    /// if the type for this data is not set, or as
    /// `fn(command: &C, choice: &CommandChoice<&'static str) -> bool`
    /// where `C` is the right hand side of `#[command(type = ...)]` on the struct if
    pub retain: Option<Path>,
    /// The description of this `DataOption`
    pub desc: Option<LitStr>,
}

#[derive(Debug)]
pub enum FieldIdent {
    Named(NamedField),
    Unnamed(UnnamedField),
}

impl Spanned for FieldIdent {
    fn span(&self) -> Span {
        match self {
            Self::Named(named) => named.ident.span(),
            Self::Unnamed(unnamed) => unnamed.index.span,
        }
    }
}

impl FieldIdent {
    fn builder_ident(&self) -> Ident {
        match self {
            Self::Named(named) => Ident::new(&named.ident.to_string(), named.ident.span()),
            Self::Unnamed(UnnamedField { index }) => Ident::new(&format!("_{}", index.index), index.span)
        }
    }
}

#[derive(Debug)]
pub struct NamedField {
    ident: Ident,
    pub rename: Option<LitStr>,
}

#[derive(Debug)]
pub struct UnnamedField {
    index: Index,
    // todo presumably rename
}

#[derive(Debug, Default)]
pub struct Vararg {
    /// the root of the vararg parameter, for example `player` for player1, player2, player3, ...
    pub root: Option<LitStr>,
    /// `fn<C: SlashCommand>(command: &C) -> usize` to pick how many vararg options to display
    pub num: VarargNum,
    // todo make count hold the name & get rid of root, then just make the vararg field in struct optional
    /// how to name the vararg options
    pub names: VarargNames,
    /// how many varargs are required. If `None`, all are required
    pub required: Option<usize>,
}

impl Vararg {
    pub const fn is_some(&self) -> bool {
        self.root.is_some()
    }
}

#[derive(Debug)]
pub enum VarargNum {
    Count(usize),
    Function(Path),
}

impl VarargNum {
    fn take_n(&self) -> TokenStream2 {
        match self {
            VarargNum::Count(n) => quote! { #n },
            VarargNum::Function(path) => quote! { #path(command) },
        }
    }
}

impl Default for VarargNum {
    fn default() -> Self {
        Self::Count(0)
    }
}

#[derive(Debug)]
pub enum VarargNames {
    /// if root is `player`, names will be `player1`, `player2`, etc
    Index,
    /// names will be `first`, `second`, `third`, etc (up to 20)
    Ordinals,
    /// names given by this function, callable as `fn<C: Into<Cow<'static, str>>(n: usize) -> C`
    Function(Path),
}

impl Default for VarargNames {
    fn default() -> Self {
        Self::Index
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


impl VarargNames {
    fn ordinals_array() -> TokenStream2 {
        quote! {
            [
                "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth",
                "ninth", "tenth", "eleventh", "twelfth", "thirteenth", "fourteenth", "fifteenth",
                "sixteenth", "seventeenth", "eighteenth", "nineteenth", "twentieth", "twenty-first",
                "twenty-second", "twenty-third", "twenty-fourth", "twenty-fifth",
            ]
        }
    }

    fn names(&self, root: &LitStr) -> TokenStream2 {
        match self {
            VarargNames::Index => quote! {
                (1..).map(|i| format!(concat!(#root, "{}"), i))
            },
            VarargNames::Ordinals => {
                let ordinals = Self::ordinals_array();
                quote! {
                    ::std::array::IntoIter::new(#ordinals)
                }
            }
            VarargNames::Function(fun) => quote! {
                (1..).map(|i| #fun(i))
            },
        }
    }

    /// Determine if an option is part of a vararg.
    /// Called as the body of a closure with parameters `option_name: &str` and `idx: usize`, returns `bool`
    fn matches_vararg(&self, root: &LitStr) -> TokenStream2 {
        match self {
            VarargNames::Index => quote! {
                option_name.strip_prefix(#root)
                    .and_then(|num| num.parse::<usize>().ok())
                    == Some(idx)
            },
            VarargNames::Ordinals => {
                let ordinals = Self::ordinals_array();
                quote! {
                    #ordinals[idx - 1] == option_name
                }
            }
            VarargNames::Function(fun) => quote! {
                #fun(idx) == option_name
            },
        }
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<syn::Field> for Field {
    fn from(field: syn::Field) -> Self {
        let attrs = field.attrs;
        let mut field = Self {
            name: FieldIdent::Named(NamedField {
                ident: field.ident.expect("expected named fields"),
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
    pub command_type: Option<Type>,
}

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

    pub fn impl_from_options(&self, return_type: &Ident, return_ctor: &Path, command_ty: &TokenStream2) -> TokenStream2 {
        let num_fields = self.fields.len();
        let builder_struct = self.builder_struct(return_type, return_ctor);
        let varargs_match = self.varargs_match(command_ty);
        let fields_array = self.fields_array();
        let fields_match = self.match_branches(command_ty);
        let varargs_array = self.varargs_array();

        let build_struct = if self.fields.is_empty() {
            // if there are no fields (ie, is Unit struct), don't have to parse any options
            TokenStream2::new()
        } else {
            quote! {
                const FIELDS: [&'static str; #num_fields] = #fields_array;
                const VARARGS: [fn(&str, usize) -> bool; #num_fields] = #varargs_array;

                let mut options = options;
                let mut options = options.drain(0..).peekable();
                let mut idx = 0;

                while let Some(option) = options.next() {
                    // this first ^ option is always a single option or the first of a vararg
                    let matches_vararg = VARARGS[idx];
                    if matches_vararg(&option.name, 1) {
                        // start of a vararg
                        let mut varargs = vec![option];
                        let mut vararg_idx = 1;
                        loop {
                            let next_is_vararg = matches!(
                                options.peek(),
                                Some(next) if {
                                    vararg_idx += 1;
                                    matches_vararg(&next.name, vararg_idx)
                                }
                            );
                            if next_is_vararg {
                                varargs.push(options.next().unwrap());
                            } else {
                                break
                            }
                        }
                        #[allow(clippy::used_underscore_binding)]
                        #varargs_match
                    } else {
                        // todo move this in to fields_match too
                        match idx {
                            #fields_match
                            // there were too many args
                            _ => return Err(CommandParseError::BadOrder(option.name, idx, 0..#num_fields))
                        }
                    }
                    idx += 1;
                }
            }
        };
        quote! {
            use ::discorsd::errors::*;

            // declares the type and makes a mutable instance named `builder`
            #builder_struct

            // parses the command from Discord & fills `builder`
            #build_struct

            builder.build()
        }
    }

    fn fields_array(&self) -> TokenStream2 {
        let fields = self.fields.iter().map(Field::arg_name);
        quote! { [#(#fields),*] }
    }

    fn match_branches(&self, command_ty: &TokenStream2) -> TokenStream2 {
        let branches = self.fields.iter().enumerate().map(|(i, f)| {
            let builder_ident = f.name.builder_ident();
            let ty = &f.ty;
            if f.vararg.is_some() {
                quote_spanned! { ty.span() =>
                    #i => return Err(CommandParseError::UnexpectedVararg(option.name, idx))
                }
            } else {
                quote_spanned! { ty.span() =>
                    #i => builder.#builder_ident = Some(
                        <#ty as ::discorsd::commands::CommandData<#command_ty>>::from_options(option)?
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

    fn varargs_match(&self, command_ty: &TokenStream2) -> TokenStream2 {
        let branches = self.fields.iter().enumerate().map(|(i, f)| {
            let ty = &f.ty;
            let builder_ident = f.name.builder_ident();
            if f.vararg.is_some() {
                quote_spanned! { ty.span() =>
                    #i => {
                        let varargs = <#ty as ::discorsd::commands::CommandData<#command_ty>>::from_options(varargs)?;
                        builder.#builder_ident = Some(varargs);
                    }
                }
            } else {
                quote_spanned! { ty.span() =>
                    #i => return Err(CommandParseError::UnexpectedSingleOption(FIELDS[idx].to_owned(), idx))
                }
            }
        });
        let num_fields = self.fields.len();
        quote! {
            match idx {
                #(#branches,)*
                // todo specific vararg error type
                _ => return Err(CommandParseError::BadOrder(
                    FIELDS[idx].to_owned(), idx, 0..#num_fields
                ))
            }
        }
    }

    fn varargs_array(&self) -> TokenStream2 {
        let vararg_names = self.fields.iter().map(|f| {
            if f.vararg.is_some() {
                let fn_body = f.vararg.names.matches_vararg(f.vararg.root.as_ref().unwrap());
                quote_spanned! { f.name.span() => |option_name, idx| { #fn_body } }
            } else {
                quote_spanned! { f.name.span() => |_, _| false }
            }
        });
        quote! { [#(#vararg_names),*] }
    }

    pub fn data_options(&self, command_type: &TokenStream2) -> TokenStream2 {
        let chain = self.fields.iter().map(|f| {
            if f.vararg.is_some() {
                f.vararg_option(command_type)
            } else {
                let name = f.arg_name();
                let desc = f.desc.as_ref()
                    .map_or_else(|| f.arg_name(), LitStr::value);
                let single_option = f.single_option(Some((name, desc)), None);
                quote_spanned! { single_option.span() =>
                    ::std::iter::once(#single_option)
                }
            }
        }).reduce(|a, b| {
            quote_spanned! { a.span() =>
                #a.chain(#b)
            }
        }).unwrap_or_else(|| quote! { ::std::iter::empty() });
        quote_spanned! { chain.span() => #chain.collect() }
    }
}

impl Field {
    /// data options for not varargs
    fn single_option(
        &self,
        name_desc: Option<(String, String)>,
        required_if_i_less_than: Option<usize>,
    ) -> TokenStream2 {
        let let_name_desc = if let Some((name, desc)) = name_desc {
            quote! {
                let name = #name;
                let desc = #desc;
            }
        } else {
            TokenStream2::new()
        };
        let required = if let Some(less_than) = required_if_i_less_than {
            quote! {
                if i < #less_than { option = option.required() }
            }
        } else if self.default.is_none() {
            quote! {
                option = option.required();
            }
        } else if let Some(required) = &self.required {
            quote! {
                if #required(command) {
                    option = option.required();
                }
            }
        } else {
            TokenStream2::new()
        };
        // this still might not be perfect
        let ty = self.ty.generic_type()
            .or_else(|| self.ty.array_type())
            .unwrap_or(&self.ty);
        let retain = if let Some(path) = &self.retain {
            quote_spanned! { path.span() =>
                choices.retain(|choice| #path(command, choice));
            }
        } else {
            TokenStream2::new()
        };
        quote_spanned! { self.name.span() =>
            {
                #let_name_desc
                #[allow(unused_mut)]
                let mut choices = <#ty>::make_choices(command);
                #retain
                if choices.is_empty() {
                    #[allow(unused_mut)]
                    let mut option = ::discorsd::commands::CommandDataOption::new(name, desc);
                    #required
                    <#ty as ::discorsd::commands::OptionCtor>::option_ctor(option)
                } else {
                    #[allow(unused_mut)]
                    let mut option = ::discorsd::commands::CommandDataOption::new_str(name, desc)
                                        .choices(choices);
                    #required
                    ::discorsd::commands::DataOption::String(option)
                }
            }
        }
    }

    fn vararg_option(&self, command_type: &TokenStream2) -> TokenStream2 {
        let ty = &self.ty;
        let root = self.vararg.root.as_ref().unwrap();
        let names = self.vararg.names.names(root);
        let take = self.vararg.num.take_n();
        // todo make this settable too, default to ?names?
        let descriptions = VarargNames::Index.names(root);
        let single_opt = self.single_option(None, self.vararg.required);

        quote! {
            // `command` is in scope from `CommandData::make_args` param
            #names
                .take(
                    <#ty as ::discorsd::commands::CommandData<#command_type>>::vararg_number()
                        .number()
                        .unwrap_or(#take)
                )
                .zip(#descriptions)
                .enumerate()
                .map(|(i, (name, desc))| #single_opt)
        }
    }
}

impl FromIterator<Field> for Struct {
    fn from_iter<I: IntoIterator<Item=Field>>(iter: I) -> Self {
        let fields: Vec<Field> = iter.into_iter().collect();
        Self { fields, command_type: None }
    }
}
