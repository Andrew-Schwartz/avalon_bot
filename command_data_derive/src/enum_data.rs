use std::convert::TryFrom;
use std::iter::FromIterator;

use proc_macro2::TokenStream as TokenStream2;
use proc_macro_error::*;
use quote::{quote, quote_spanned};
use syn::{Attribute, DataEnum, Fields, Ident, LitStr, Type};
use syn::spanned::Spanned;

use crate::struct_data::Struct;
use crate::utils::command_data_impl;

pub fn enum_impl(ty: &Ident, data: DataEnum, attrs: &[Attribute]) -> TokenStream2 {
    let mut variants: Enum = data.variants
        .into_iter()
        .map(Variant::from)
        .collect();
    for attr in attrs {
        if !attr.path.is_ident("command") { continue; };
        variants.handle_attribute(attr);
    }
    variants.args_maker_impl(ty)
}

#[derive(Debug)]
pub struct Variant {
    attrs: Vec<Attribute>,
    ident: Ident,
    pub rename: Option<LitStr>,
    fields: Fields,
    pub desc: Option<LitStr>,
}

impl Variant {
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

impl From<syn::Variant> for Variant {
    fn from(variant: syn::Variant) -> Self {
        if variant.discriminant.is_some() {
            abort!(variant, "Command variants can't have discriminants (ex, `= 1`)");
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
            variant.handle_attribute(attr);
        }
        variant.attrs = attrs;

        variant
    }
}

#[derive(Debug)]
pub struct Enum {
    variants: Vec<Variant>,
    /// settable with `#[command(type = MyCommand)]` on an enum
    pub command_type: Option<Type>,
    pub rename_all: Option<RenameAll>,
}

// todo more of these ig
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RenameAll {
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
    //noinspection RsSelfConvention
    fn from_options_branches(&self, ty: &Ident, command_ty: &TokenStream2) -> TokenStream2 {
        let branches = self.variants.iter().enumerate().map(|(n, v)| {
            let patt = v.name(self.rename_all);
            // todo filter out the attributes this used
            let fields = Struct::from_fields(v.fields.clone(), &[]);
            match syn::parse_str(&format!("{}::{}", ty, v.ident)) {
                Ok(path) => {
                    let try_from_body = fields.impl_from_options(ty, &path, command_ty, n);
                    quote_spanned! { v.ident.span() =>
                        #patt => {
                            #try_from_body
                        }
                    }
                }
                Err(e) => abort!(e),
            }
        });
        quote! {
            #(#branches,)*
        }
    }

    fn make_args_vec(&self) -> TokenStream2 {
        let branches = self.variants.iter().map(|v| {
            // todo filter out the attributes this used
            let strukt = Struct::from_fields(v.fields.clone(), &[]);
            let name = v.name(self.rename_all);
            let desc = v.description(&name);
            let options = strukt.data_options();
            quote_spanned! { v.ident.span() =>
                Self::VecArg::make(
                    #name, #desc, #options
                )
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

    fn args_maker_impl(&self, ty: &Ident) -> TokenStream2 {
        let differ_err = |variant: &Variant| abort!(
            variant.fields,
            "All variants must be same type (tuple/struct), but this one isn't",
        );

        let mut inline_data = None::<bool>;
        for variant in &self.variants {
            if let Some(inline_data) = inline_data {
                // make sure each variant is same type
                match &variant.fields {
                    Fields::Named(_) => {
                        if !inline_data {
                            differ_err(variant);
                        }
                    }
                    Fields::Unnamed(f) => {
                        if f.unnamed.len() == 1 {
                            if inline_data {
                                differ_err(variant);
                            }
                        } else if !inline_data {
                            differ_err(variant);
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
            None => abort_call_site!("Empty enums can't be Command Data"),
            Some(true) => self.inline_structs(ty),
            Some(false) => self.newtype_structs(ty),
        }
    }

    /// Enums where each variant is a newtype
    /// ```
    /// # const IGNORE1: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// struct Color { hex: String }
    /// # const IGNORE2: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// struct Person { name: String, age: u32 }
    ///
    /// # const IGNORE3: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
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
        let first_variant_ty = &self.variants.iter()
            .find(|v| !matches!(&v.fields, Fields::Unit))
            .expect("Enum is not empty")
            .fields.iter()
            .next()
            .expect("All newtype enums have at least one newtype")
            .ty;
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

    /// Enums where each variant is either a struct or a tuple with 2+ fields (that tuple thing
    /// might be a lie as to how well it works for tuples...)
    /// ```
    /// # const IGNORE: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// enum InlineStructs {
    ///     ColorCommand { hex: String },
    ///     PersonCommand(
    /// # #[doc = r#"
    ///         #[command(rename = "name")]
    /// # "#]
    ///         String,
    /// # #[doc = r#"
    ///         #[command(rename = "age")]
    /// # "#]
    ///         u32
    ///     )
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