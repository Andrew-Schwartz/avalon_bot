use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;

use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use syn::{Attribute, DataEnum, Fields, Ident, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta, Type};
use syn::spanned::Spanned;

use crate::struct_data::Struct;
use crate::utils::command_data_impl;

// todo use this in other files too
macro_rules! comp_err {
    ($($tt:tt)*) => {
        match $($tt)* {
            Ok(ok) => ok,
            Err(err) => return err.into_compile_error(),
        }
    };
}

pub fn enum_impl(ty: &Ident, data: DataEnum, attrs: &[Attribute]) -> TokenStream2 {
    let variants: Result<Enum, syn::Error> = data.variants
        .into_iter()
        .map(Variant::try_from)
        .collect();
    let mut variants = comp_err!(variants);
    for attr in attrs {
        if !attr.path.is_ident("command") { continue; };
        comp_err!(variants.handle_attribute(attr));
    }
    comp_err!(variants.args_maker_impl(ty))
}

#[derive(Debug)]
struct Variant {
    attrs: Vec<Attribute>,
    ident: Ident,
    rename: Option<LitStr>,
    // todo will probably have to be Vec<My::Field>
    fields: Fields,
    desc: Option<LitStr>,
}

impl Variant {
    // fn lowercase_name(&self) -> LitStr {
    //     LitStr::new(&self.ident.to_string().to_lowercase(), self.ident.span())
    // }

    fn name(&self, rename_all: Option<RenameAll>) -> String {
        if let Some(lit) = &self.rename {
            lit.value()
        } else if let Some(rename_all) = rename_all {
            rename_all.rename(&self.ident)
        } else {
            self.ident.to_string()
        }
    }

    fn description(&self, name: &str) -> String {
        self.desc.as_ref()
            .map_or_else(|| name.to_string(), LitStr::value)
    }
}

impl TryFrom<syn::Variant> for Variant {
    type Error = syn::Error;

    fn try_from(variant: syn::Variant) -> Result<Self, Self::Error> {
        if variant.discriminant.is_some() {
            return Err(syn::Error::new(variant.span(), "Command variants can't have discriminants (ex, `= 1`)"));
        }
        let attrs = variant.attrs;
        let mut variant = Self {
            attrs: Vec::new(),
            ident: variant.ident,
            rename: None,
            fields: variant.fields,
            desc: None,
        };
        for attr in &attrs {
            if !attr.path.is_ident("command") { continue; }
            variant.handle_attribute(attr)?;
        }
        variant.attrs = attrs;

        Ok(variant)
    }
}

impl Variant {
    fn handle_attribute(&mut self, attr: &Attribute) -> Result<(), syn::Error> {
        let meta = attr.parse_meta()?;
        match meta {
            Meta::List(MetaList { nested, .. }) => {
                for n in nested {
                    match n {
                        NestedMeta::Meta(
                            Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. })
                        ) => {
                            if path.is_ident("desc") {
                                self.desc = Some(str);
                            } else if path.is_ident("rename") {
                                self.rename = Some(str);
                            } else {
                                return Err(syn::Error::new(
                                    str.span(),
                                    format!("Unknown attribute `{:?}`", str),
                                ));
                            }
                        }
                        other => return Err(syn::Error::new(
                            other.span(),
                            format!("Unexpected NestedMeta {:?}", other),
                        ))
                    }
                }
            }
            other => return Err(syn::Error::new(
                other.span(),
                format!("unexpected meta {:?}", other),
            )),
        };
        Ok(())
    }
}

#[derive(Debug)]
struct Enum {
    variants: Vec<Variant>,
    /// settable with `#[command(type = MyCommand)]` on an enum
    command_type: Option<Type>,
    rename_all: Option<RenameAll>,
}

// todo more of these ig
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum RenameAll {
    Lowercase,
}

impl RenameAll {
    pub fn rename(self, ident: &Ident) -> String {
        match self {
            RenameAll::Lowercase => ident.to_string().to_lowercase(),
        }
    }
}

impl TryFrom<LitStr> for RenameAll {
    type Error = syn::Error;

    fn try_from(value: LitStr) -> Result<Self, Self::Error> {
        match value.value().as_str() {
            "lowercase" => Ok(Self::Lowercase),
            bad => Err(syn::Error::new(
                value.span(),
                format!("{} is not a supported `rename_all` option. Try one of [Lowercase]", bad),
            ))
        }
    }
}

impl Enum {
    fn handle_attribute(&mut self, attr: &Attribute) -> Result<(), syn::Error> {
        let meta = attr.parse_meta()?;
        match meta {
            Meta::List(MetaList { nested, .. }) => {
                for n in nested {
                    match n {
                        NestedMeta::Meta(
                            Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. })
                        ) => {
                            if path.is_ident("type") {
                                self.command_type = Some(str.parse()?);
                            } else if path.is_ident("rename_all") {
                                self.rename_all = Some(str.try_into()?);
                            } else {
                                return Err(syn::Error::new(
                                    str.span(),
                                    format!("Unknown attribute `{:?}`", str),
                                ));
                            }
                        }
                        other => return Err(syn::Error::new(
                            other.span(),
                            format!("Unexpected NestedMeta {:?}", other),
                        ))
                    }
                }
            }
            other => return Err(syn::Error::new(
                other.span(),
                format!("unexpected meta {:?}", other),
            )),
        };
        Ok(())
    }

    //noinspection RsSelfConvention
    fn from_options_branches(&self, ty: &Ident, command_ty: &TokenStream2) -> TokenStream2 {
        let branches = self.variants.iter().enumerate().map(|(n, v)| {
            let patt = v.name(self.rename_all);
            match Struct::from_fields(v.fields.clone(), &v.attrs) {
                Ok(fields) => match syn::parse_str(&format!("{}::{}", ty, v.ident)) {
                    Ok(path) => {
                        let try_from_body = fields.impl_from_options(ty, &path, command_ty, n);
                        quote_spanned! { v.ident.span() =>
                            #patt => {
                                #try_from_body
                            }
                        }
                    }
                    Err(e) => e.into_compile_error(),
                },
                Err(e) => e.into_compile_error(),
            }
        });
        quote! {
            #(#branches,)*
        }
    }

    fn make_args_vec(&self) -> TokenStream2 {
        let branches = self.variants.iter().map(|v| {
            match Struct::from_fields(v.fields.clone(), &v.attrs) {
                Ok(strukt) => {
                    let name = v.name(self.rename_all);
                    let desc = v.description(&name);
                    let options = strukt.data_options();
                    quote_spanned! { v.ident.span() =>
                        Self::VecArg::make(
                            #name, #desc, #options
                        )
                    }
                }
                Err(e) => e.into_compile_error(),
            }
        });

        quote! {
            vec![#(#branches,)*]
        }
    }

    fn variants_array(&self) -> TokenStream2 {
        let array = self.variants.iter()
            .map(|v| LitStr::new(&v.ident.to_string().to_lowercase(), v.ident.span()));
        quote! { [#(#array),*] }
    }

    fn args_maker_impl(&self, ty: &Ident) -> Result<TokenStream2, syn::Error> {
        let differ_err = |variant: &Variant| Err(syn::Error::new(
            variant.fields.span(),
            "All variants must be same type (tuple/struct), but this one isn't",
        ));

        let mut inline_data = None::<bool>;
        for variant in &self.variants {
            if let Some(inline_data) = inline_data {
                // make sure each variant is same type
                match &variant.fields {
                    Fields::Named(_) => {
                        if !inline_data {
                            return differ_err(variant);
                        }
                    }
                    Fields::Unnamed(f) => {
                        if f.unnamed.len() == 1 {
                            if inline_data {
                                return differ_err(variant);
                            }
                        } else if !inline_data {
                            return differ_err(variant);
                        }
                    }
                    // just skip unit structs
                    Fields::Unit => {}
                }
            } else {
                // first variant, set `inline_data`
                match &variant.fields {
                    Fields::Named(_) => inline_data = Some(true),
                    Fields::Unnamed(f) => {
                        if f.unnamed.len() == 1 {
                            inline_data = Some(false)
                        } else {
                            inline_data = Some(true)
                        }
                    }
                    Fields::Unit => {}
                }
            }
        }

        match inline_data {
            None => Err(syn::Error::new(
                Span::call_site(),
                "Empty enums can't be Command Data",
            )),
            Some(true) => Ok(self.inline_structs(ty)),
            Some(false) => Ok(self.newtype_structs(ty)),
        }
    }

    /// Enums where each variant is a newtype
    /// ```rust
    /// #[derive(CommandData)]
    /// struct Color { hex: String }
    /// #[derive(CommandData)]
    /// struct Person { name: String, age: u32 }
    ///
    /// #[derive(CommandData)]
    /// enum Data {
    ///     ColorCommand(String),
    ///     PersonCommand(Person),
    /// #   /*
    ///     ...
    /// #   */
    /// }
    /// ```
    /// This also works if the inner of the newtype is an enum, as long as you `#[derive(CommandData)]`
    fn newtype_structs(&self, ty: &Ident) -> TokenStream2 {
        let (args_impl_statement, c_ty) = command_data_impl(self.command_type.as_ref());
        let first_variant_ty = &self.variants.get(0).expect("Enum is not empty")
            // todo some can be units, have to skip past that
            .fields.iter().next().expect("All variants are newtypes").ty;
        let args = self.variants.iter().map(|v| {
            let name = v.name(self.rename_all);
            let desc = v.description(&name);
            let make_args = if let Some(f) = &v.fields.iter().next() {
                let new_ty = &f.ty;
                quote_spanned! { new_ty.span() => <#new_ty>::make_args(command) }
            } else {
                quote! { Vec::new() }
            };
            let quote = quote_spanned! { v.ident.span() =>
                Self::VecArg::make(#name, #desc, #make_args)
            };
            quote
        });
        // let match_branches = self.match_branches(ty, &c_ty);
        let match_branches = self.variants.iter().map(|v| {
            let name = v.name(self.rename_all);
            let ident = &v.ident;
            let variant = if let Some(first) = &v.fields.iter().next() {
                let ty = &first.ty;
                quote_spanned! { first.span() =>
                    #ident(
                        <#ty as ::discorsd::model::commands::CommandData<#c_ty>>::from_options(lower)?
                    )
                }
            } else {
                quote_spanned! { ident.span() =>
                    #ident
                }
            };
            quote_spanned! { v.ident.span() =>
                #name => Ok(Self::#variant)
            }
        });
        let variants_array = self.variants_array();

        quote! {
            #args_impl_statement for #ty {
                // god that's ugly v2
                type Options =
                <
                    <
                        #first_variant_ty as ::discorsd::model::commands::CommandData<#c_ty>
                    >::Options as ::discorsd::model::commands::OptionsLadder
                >::Raise;

                fn from_options(
                    Self::Options { name, lower }: Self::Options,
                ) -> ::std::result::Result<Self, ::discorsd::errors::CommandParseError> {
                    match name.as_str() {
                        #(#match_branches,)*
                        _ => Err(::discorsd::errors::CommandParseError::UnknownOption(
                            ::discorsd::errors::UnknownOption { name, options: &#variants_array }
                        ))
                    }
                }

                // god that's ugly
                type VecArg =
                <
                    <
                        #first_variant_ty as ::discorsd::model::commands::CommandData<#c_ty>
                    >::VecArg as ::discorsd::model::commands::VecArgLadder
                >::Raise;

                fn make_args(command: &#c_ty) -> ::std::vec::Vec<Self::VecArg> {
                    vec![#(#args),*]
                }
            }
        }
    }

    /// Enums where each variant is a struct or a tuple with 2+ fields
    /// ```
    /// #[derive(CommandData)]
    /// enum InlineStructs {
    ///     ColorCommand { hex: String },
    ///     PersonCommand(#[command(rename = "name")] String, #[command(rename = "age")] u32)
    /// }
    /// ```
    fn inline_structs(&self, ty: &Ident) -> TokenStream2 {
        let (command_data_impl_statement, c_ty) = command_data_impl(self.command_type.as_ref());
        let from_option_branches = self.from_options_branches(ty, &c_ty);
        let variants_array = self.variants_array();
        let make_args_vec = self.make_args_vec();

        quote! {
            #command_data_impl_statement for #ty {
                // All inline struct enums are SubCommands
                type Options = ::discorsd::model::interaction::CommandOption;

                fn from_options(
                    Self::Options { name, lower: options }: Self::Options
                ) -> ::std::result::Result<Self, ::discorsd::errors::CommandParseError> {
                    use ::discorsd::errors::*;
                    match name.as_str() {
                        #from_option_branches
                        _ => Err(CommandParseError::UnknownOption(UnknownOption {
                            name,
                            options: &#variants_array,
                        }))
                    }
                }

                // All inline struct enums are SubCommands
                type VecArg = ::discorsd::model::interaction::SubCommand;

                fn make_args(command: &#c_ty) -> ::std::vec::Vec<Self::VecArg> {
                    #make_args_vec
                }
            }
        }
    }
}

impl FromIterator<Variant> for Enum {
    fn from_iter<T: IntoIterator<Item=Variant>>(iter: T) -> Self {
        Self { variants: iter.into_iter().collect(), command_type: None, rename_all: None }
    }
}