use std::iter::FromIterator;

use proc_macro2::TokenStream as TokenStream2;
use proc_macro_error::*;
use quote::{quote, quote_spanned};
use syn::{DataEnum, Ident, LitStr};

use crate::utils::IteratorJoin;

pub fn enum_impl(ty: &Ident, data: DataEnum) -> TokenStream2 {
    let variants: Enum = data.variants
        .into_iter()
        .map(Variant::from)
        .collect();
    let choices = variants.choices();
    let branches = variants.branches();
    let variants_array = variants.array();
    let default_impl = variants.default_impl(ty);
    let eq_branches = variants.eq_branches();

    let tokens = quote! {
        impl ::discorsd::commands::OptionCtor for #ty {
            type Data = &'static str;

            fn option_ctor(
                cdo: ::discorsd::commands::CommandDataOption<Self::Data>
            ) -> ::discorsd::commands::DataOption {
                ::discorsd::commands::DataOption::String(cdo)
            }
        }

        // todo this probably has to be able to be specialized too
        //  maybe not?
        impl<C: ::discorsd::commands::SlashCommandRaw> ::discorsd::model::commands::CommandData<C> for #ty {
            // all choice enums are built from a ValueOption
            type Options = ::discorsd::model::interaction::ValueOption;

            fn from_options(
                Self::Options { name, lower: value }: Self::Options,
            ) -> ::std::result::Result<Self, ::discorsd::errors::CommandParseError> {
                use ::discorsd::errors::*;
                let value = value.string()?;
                match value.as_str() {
                    #branches
                    _ => ::std::result::Result::Err(CommandParseError::UnknownOption(UnknownOption {
                        name: value, options: &#variants_array
                    }))
                }
            }

            type VecArg = ::discorsd::commands::DataOption;

            fn make_args(_: &C) -> Vec<Self::VecArg> { Vec::new() }
            fn make_choices(_: &C) -> Vec<::discorsd::model::interaction::CommandChoice<&'static str>> {
                vec![#choices]
            }
        }

        #default_impl

        impl<'a> PartialEq<&'a str> for #ty {
            fn eq(&self, other: &&'a str) -> bool {
                match self {
                    #eq_branches
                }
            }
        }

        impl<'a> PartialEq<#ty> for &'a str {
            fn eq(&self, other: &#ty) -> bool {
                other == self
            }
        }

        impl PartialEq<str> for #ty {
            fn eq(&self, other: &str) -> bool {
                self == &other
            }
        }

        impl PartialEq<#ty> for str {
            fn eq(&self, other: &#ty) -> bool {
                other == &self
            }
        }
    };
    tokens
}

#[derive(Debug)]
pub struct Variant {
    ident: Ident,
    pub choice: Option<LitStr>,
    pub default: bool,
}

impl Variant {
    fn name(&self) -> LitStr {
        LitStr::new(&self.ident.to_string(), self.ident.span())
    }
}

impl From<syn::Variant> for Variant {
    fn from(variant: syn::Variant) -> Self {
        if !variant.fields.is_empty() {
            abort!(variant, "Command variants can't have fields")
        }
        if variant.discriminant.is_some() {
            abort!(variant, "Command variants can't have discriminants (ex, `= 1`)")
        }
        let attrs = variant.attrs;
        let mut variant = Self { ident: variant.ident, choice: None, default: false };
        attrs.iter()
            .filter(|a| a.path.is_ident("command"))
            .for_each(|a| variant.handle_attribute(a));
        variant
    }
}

#[derive(Debug)]
struct Enum(Vec<Variant>);

impl Enum {
    fn choices(&self) -> TokenStream2 {
        let choices = self.0.iter().map(|v| {
            let name = v.choice.as_ref().map_or_else(|| v.ident.to_string(), LitStr::value);
            let span = v.ident.span();
            let value = v.name();
            quote_spanned! { span => ::discorsd::model::interaction::CommandChoice::new(#name, #value) }
        });
        quote! {
            #(#choices),*
        }
    }

    fn branches(&self) -> TokenStream2 {
        let branches = self.0.iter().map(|v| {
            let str = v.name();
            let ident = &v.ident;
            quote_spanned! { v.ident.span() => #str => ::std::result::Result::Ok(Self::#ident) }
        });
        quote! {
            #(#branches,)*
        }
    }

    fn array(&self) -> TokenStream2 {
        let array = self.0.iter().map(Variant::name);
        quote! { [#(#array),*] }
    }

    fn default_impl(&self, ty: &Ident) -> TokenStream2 {
        let defaults: Vec<_> = self.0.iter()
            .filter(|v| v.default)
            .map(|v| &v.ident)
            .collect();
        match defaults.as_slice() {
            [] => TokenStream2::new(),
            [variant] => quote! {
                impl std::prelude::v1::Default for #ty {
                    fn default() -> Self {
                        Self::#variant
                    }
                }
            },
            too_long => {
                let variants = too_long.iter().join(", ");
                abort!(
                    ty,
                    format!("Only one variant can be marked default (`{}` all are)", variants),
                )
            }
        }
    }

    fn eq_branches(&self) -> TokenStream2 {
        let branches = self.0.iter().map(|v| {
            let ident = &v.ident;
            let name = v.name();
            quote_spanned! { v.ident.span() => Self::#ident => *other == #name }
        });
        quote! {
            #(#branches,)*
        }
    }
}

impl FromIterator<Variant> for Enum {
    fn from_iter<T: IntoIterator<Item=Variant>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}
