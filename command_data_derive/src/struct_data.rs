use std::iter::FromIterator;

use proc_macro2::{Span, TokenStream as TokenStream2};
use proc_macro_error::emit_error;
use quote::{quote, quote_spanned};
use syn::{Attribute, Fields, Ident, Index, LitStr, Path, Type, TypeParam};
use syn::spanned::Spanned;

use crate::utils::*;

pub fn struct_impl(ty: &Ident, generics: Vec<TypeParam>, fields: Fields, attributes: &[Attribute]) -> TokenStream2 {
    let generic_ty = {
        let generic_names = generics.iter().map(|g| {
            let ident = &g.ident;
            quote_spanned! { ident.span() => #ident }
        });
        quote_spanned! { ty.span() => #ty<#(#generic_names),*> }
    };
    let strukt = Struct::from_fields(fields, attributes, generics);
    let (command_data_impl, command_type) = command_data_impl(strukt.command_type.as_ref(), &strukt.generics);
    let from_options_body = strukt.impl_from_options(
        &generic_ty,
        &Path::from(ty.clone()),
        &command_type,
    );
    let data_options = strukt.data_options(&command_type);

    let tokens = quote! {
        #command_data_impl for #generic_ty {
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

            type Choice = Self;
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
    /// struct's type is specified (`#[command(type = "MyCommand")]`)
    pub required: Option<Path>,
    /// see [Vararg](Vararg) for details
    pub vararg: Option<Vararg>,
    // todo now uses the enum type
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

    fn ident(&self) -> TokenStream2 {
        match self {
            FieldIdent::Named(NamedField { ident, .. }) => quote! { #ident },
            FieldIdent::Unnamed(UnnamedField { index }) => quote! { #index },
        }
    }
}

#[derive(Debug)]
pub struct NamedField {
    ident: Ident,
    pub rename: Option<LitStr>,
}

impl NamedField {
    fn rename_with_generics(&self, generics: &[TypeParam]) -> Option<TokenStream2> {
        if let Some(rename) = &self.rename {
            let mut format_string = rename.value();
            replace_generics(&mut format_string, generics)
                .or_else(|| Some(quote! { #format_string }))
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub struct UnnamedField {
    index: Index,
    // todo presumably rename
    //  NO THAT WOULDN'T MAKE SENSE FOR AN UNNAMED FIELD IF YOU THINK ABOUT IT
}

#[derive(Debug, Default)]
pub struct Vararg {
    /// how to name the vararg options
    pub names: VarargNames,
    /// `fn<C: SlashCommand>(command: &C) -> usize` to pick how many vararg options to display
    pub num: VarargNum,
    /// how many varargs are required. If `None`, all are required
    pub required: Option<usize>,
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
    /// names will be `first`, `second`, `third`, etc
    Ordinals,
    /// if the `LitStr` is "player", the names will be `player1`, `player2`, etc
    Index(LitStr),
    /// names given by this function, callable as `fn<C: Into<Cow<'static, str>>(n: usize) -> C`
    Function(Path),
}

impl Default for VarargNames {
    fn default() -> Self {
        Self::Ordinals
    }
}

impl VarargNames {
    const fn as_index(&self) -> Option<&LitStr> {
        match self {
            Self::Index(root) => Some(root),
            _ => None,
        }
    }
}

impl Field {
    fn arg_name(&self, generics: &[TypeParam]) -> TokenStream2 {
        match &self.name {
            FieldIdent::Named(named) => named.rename_with_generics(generics)
                .or_else(|| self.vararg.as_ref()
                    .and_then(|v| v.names.as_index())
                    .map(|str| quote! { #str }))
                .unwrap_or_else(|| {
                    let string = named.ident.to_string();
                    quote! { #string }
                }),
            FieldIdent::Unnamed(unnamed) => {
                let string = self.vararg.as_ref()
                    .and_then(|v| v.names.as_index())
                    .map_or_else(|| unnamed.index.index.to_string(), LitStr::value);
                quote! { #string }
            }
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

    fn names(&self) -> TokenStream2 {
        match self {
            VarargNames::Index(root) => quote! {
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
    fn matches_vararg(&self) -> TokenStream2 {
        match self {
            VarargNames::Index(root) => quote! {
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
    generics: Vec<TypeParam>,
}

impl Struct {
    const UNIT: Self = Self { fields: Vec::new(), command_type: None, generics: Vec::new() };

    pub fn from_fields(fields: Fields, attributes: &[Attribute], generics: Vec<TypeParam>) -> Self {
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
            Fields::Unit => Self::UNIT,
        };
        strukt.generics = generics;
        for attr in attributes {
            if !attr.path.is_ident("command") { continue; }
            strukt.handle_attribute(attr);
        }
        strukt
    }

    pub fn impl_from_options(
        &self,
        return_type: &TokenStream2,
        return_ctor: &Path,
        command_ty: &TokenStream2,
    ) -> TokenStream2 {
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
                const VARARGS: [fn(&str, usize) -> bool; #num_fields] = #varargs_array;
                let fields: [::std::borrow::Cow<'static, str>; #num_fields] = #fields_array;

                let mut options = options;
                let mut options = options.drain(0..).peekable();
                let mut idx = 0;

                while let ::std::option::Option::Some(option) = options.next() {
                    // this first ^ option is always a single option or the first of a vararg
                    let matches_vararg = VARARGS[idx];
                    if matches_vararg(&option.name, 1) {
                        // start of a vararg
                        let mut varargs = vec![option];
                        let mut vararg_idx = 1;
                        loop {
                            let next_is_vararg = matches!(
                                options.peek(),
                                ::std::option::Option::Some(next) if {
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
                        loop {
                            // maybe?
                            // #[allow(clippy::used_underscore_binding)]
                            #fields_match
                            idx += 1;
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
        let fields = self.fields.iter().map(|f| f.arg_name(&self.generics));
        quote! { [#(#fields.into()),*] }
    }

    fn match_branches(&self, command_ty: &TokenStream2) -> TokenStream2 {
        let branches = self.fields.iter().enumerate().map(|(i, f)| {
            let builder_ident = f.name.builder_ident();
            let ty = &f.ty;
            if f.vararg.is_some() {
                quote_spanned! { ty.span() =>
                    #i => return ::std::result::Result::Err(CommandParseError::UnexpectedVararg(option.name, idx))
                }
            } else {
                quote_spanned! { ty.span() =>
                    #i => if option.name == fields[#i] {
                        builder.#builder_ident = ::std::option::Option::Some(
                            <#ty as ::discorsd::commands::CommandData<#command_ty>>::from_options(option)?
                        );
                        break
                    }
                }
            }
        });
        let num_fields = self.fields.len();
        quote! {
            match idx {
                #(#branches,)*
                #num_fields => return ::std::result::Result::Err(CommandParseError::BadOrder(option.name, idx, 0..#num_fields)),
                _ => {}
            }
        }
    }

    fn builder_struct(
        &self,
        generic_return_type: &TokenStream2,
        return_ctor: &Path,
    ) -> TokenStream2 {
        let builder_prefix: String = return_ctor.segments.iter()
            .map(|seq| seq.ident.to_string())
            .collect();
        // todo span these to the declaration of the struct
        let builder_ident = Ident::new(&format!("{}Builder", builder_prefix), Span::call_site());
        let option_ctor_generic_bound = std::iter::once(quote! { ::discorsd::commands::OptionCtor });
        let bounded_generics = declaration_generics(&self.generics, option_ctor_generic_bound);
        let use_generics = use_generics(&self.generics);

        let fields = self.fields.iter().map(|f| {
            let ident = &f.name.builder_ident();
            let ty = &f.ty;
            quote_spanned! { f.name.span() => #ident: ::std::option::Option<#ty> }
        });
        let default_fields = self.fields.iter().map(|f| {
            let ident = &f.name.builder_ident();
            quote_spanned! { f.name.span() => #ident: ::std::option::Option::None }
        });
        let builder = self.fields.iter().map(|f| {
            let builder_ident = &f.name.builder_ident();
            let self_ident = f.name.ident();
            // f.name.
            // let name = self_ident.to_string();
            let missing_option_string = f.arg_name(&self.generics);
            let opt_handler = if let Some(path) = &f.default {
                quote_spanned! { path.span() => unwrap_or_else(#path) }
            } else {
                quote! {
                    ok_or_else(|| ::discorsd::errors::CommandParseError::MissingOption(
                        #missing_option_string.to_string()
                    ))?
                }
            };
            quote_spanned! { f.name.span() =>
                #self_ident: self.#builder_ident.#opt_handler
            }
        });
        quote! {
            // can't derive Default, since it has to work for generics that don't implement Default,
            // because the only default we ever want is Option::<T>::None
            struct #builder_ident<#bounded_generics> {
                #(#fields),*
            }

            impl<#bounded_generics> ::std::default::Default for #builder_ident<#use_generics> {
                // #[derive(Default)] marks it as inline, so I will too
                #[inline]
                fn default() -> Self {
                    Self { #(#default_fields),* }
                }
            }

            impl<#bounded_generics> #builder_ident<#use_generics> {
                fn build(self) -> ::std::result::Result<#generic_return_type, ::discorsd::errors::CommandParseError> {
                    #[allow(clippy::used_underscore_binding)]
                    ::std::result::Result::Ok(#return_ctor {
                        #(#builder),*
                    })
                }
            }

            let mut builder = #builder_ident::default();
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
                        builder.#builder_ident = ::std::option::Option::Some(varargs);
                    }
                }
            } else {
                quote_spanned! { ty.span() =>
                    #i => return ::std::result::Result::Err(
                        CommandParseError::UnexpectedSingleOption(fields[idx].to_string(), idx)
                    )
                }
            }
        });
        let num_fields = self.fields.len();
        quote! {
            match idx {
                #(#branches,)*
                // todo specific vararg error type
                _ => return ::std::result::Result::Err(CommandParseError::BadOrder(
                    fields[idx].to_string(), idx, 0..#num_fields
                ))
            }
        }
    }

    fn varargs_array(&self) -> TokenStream2 {
        let vararg_names = self.fields.iter().map(|f| {
            if let Some(vararg) = &f.vararg {
                let fn_body = vararg.names.matches_vararg();
                quote_spanned! { f.name.span() => |option_name, idx| { #fn_body } }
            } else {
                quote_spanned! { f.name.span() => |_, _| false }
            }
        });
        quote! { [#(#vararg_names),*] }
    }

    pub fn data_options(&self, command_type: &TokenStream2) -> TokenStream2 {
        let chain = self.fields.iter().map(|f| {
            if let Some(vararg) = &f.vararg {
                f.vararg_option(vararg, command_type)
            } else {
                let name = f.arg_name(&self.generics);
                let desc = description_len_check(&f.desc, &self.generics)
                    .unwrap_or_else(|| name.clone());
                let single_option = f.single_option(
                    Some((name, desc)),
                    None,
                    command_type,
                );
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

pub fn description_len_check(desc: &Option<LitStr>, generics: &[TypeParam]) -> Option<TokenStream2> {
    if let Some(lit_desc) = desc {
        let mut desc = lit_desc.value();
        if let Some(fmt) = replace_generics(&mut desc, generics) {
            Some(fmt)
        } else {
            if desc.is_empty() {
                emit_error!(lit_desc, "Command option descriptions can't be empty. Leave the desc \
                                   empty to use the name as the description.");
            }
            let len = desc.len();
            if len > 100 {
                emit_error!(
                lit_desc,
                "Command option descriptions can be at most 100 characters, this is {} characters",
                len,
            );
            }
            Some(quote! { #desc })
        }
    } else {
        None
    }
}

impl Field {
    /// data options for not varargs
    fn single_option(
        &self,
        name_desc: Option<(TokenStream2, TokenStream2)>,
        required_if_i_less_than: Option<usize>,
        command_type: &TokenStream2,
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
                // all choices are `Copy`
                choices.retain(|&choice| #path(command, choice));
            }
        } else {
            TokenStream2::new()
        };
        quote_spanned! { self.name.span() =>
            {
                #let_name_desc
                #[allow(unused_mut)]
                let mut choices = <#ty as ::discorsd::commands::CommandData<#command_type>>::make_choices();
                #retain
                if choices.is_empty() {
                    #[allow(unused_mut)]
                    let mut option = ::discorsd::commands::CommandDataOption::new(name, desc);
                    #required
                    <#ty as ::discorsd::commands::OptionCtor>::option_ctor(option)
                } else {
                    #[allow(unused_mut)]
                    let mut option = ::discorsd::commands::CommandDataOption::new_str(name, desc)
                                        .choices(choices.into_iter().map(<#ty as ::discorsd::commands::CommandData<#command_type>>::into_command_choice).collect());
                    #required
                    ::discorsd::commands::DataOption::String(option)
                }
            }
        }
    }

    fn vararg_option(&self, vararg: &Vararg, command_type: &TokenStream2) -> TokenStream2 {
        let ty = &self.ty;
        let names = vararg.names.names();
        let take = vararg.num.take_n();
        // todo make this settable too
        let descriptions = &names;
        let single_opt = self.single_option(None, vararg.required, command_type);

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
        Self { fields, command_type: None, generics: Vec::new() }
    }
}
