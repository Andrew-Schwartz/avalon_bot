use std::convert::TryFrom;
use std::iter::FromIterator;

use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned};
use syn::{DataEnum, Ident, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta};

use crate::utils::IteratorJoin;

pub fn enum_impl(ty: &Ident, data: DataEnum) -> TokenStream2 {
    let variants: Result<Enum, syn::Error> = data.variants
        .into_iter()
        .map(Variant::try_from)
        .collect();
    let variants = match variants {
        Ok(e) => e,
        Err(err) => return err.into_compile_error(),
    };
    let choices = variants.choices();
    let branches = variants.branches();
    let variants_array = variants.array();
    let default_impl = variants.default_impl(ty);
    let eq_branches = variants.eq_branches();

    let tokens = quote! {
        impl discorsd::model::commands::OptionChoices for #ty {
            fn choices() -> Vec<discorsd::model::interaction::CommandChoice<&'static str>> {
                vec![
                    #choices
                ]
            }
        }

        impl discorsd::model::commands::FromCommandOption for #ty {
            fn try_from(data: discorsd::model::interaction::ApplicationCommandInteractionDataOption) -> ::std::result::Result<Self, discorsd::errors::CommandParseError> {
                use discorsd::model::commands::FromCommandOption;
                use discorsd::errors::*;
                let discorsd::model::interaction::ApplicationCommandInteractionDataOption { name, value, options: _ } = data;
                let value = value
                    .ok_or_else(|| CommandParseError::EmptyOption(name))?
                    .string()?;
                match value.as_str() {
                    #branches
                    _ => Err(CommandParseError::UnknownOption(UnknownOption {
                        name: value,
                        options: &#variants_array
                    })),
                }
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
struct Variant {
    ident: Ident,
    choice: Option<LitStr>,
    default: bool,
}

impl Variant {
    fn name(&self) -> LitStr {
        LitStr::new(&self.ident.to_string(), self.ident.span())
    }
}

impl TryFrom<syn::Variant> for Variant {
    type Error = syn::Error;

    fn try_from(variant: syn::Variant) -> Result<Self, Self::Error> {
        if !variant.fields.is_empty() {
            return Err(syn::Error::new_spanned(variant, "Command variants can't have fields"));
        }
        if variant.discriminant.is_some() {
            return Err(syn::Error::new_spanned(variant, "Command variants can't have discriminants (ex, `= 1`)"));
        }
        let attrs = variant.attrs;
        let mut variant = Self { ident: variant.ident, choice: None, default: false };
        attrs.into_iter()
            .filter(|a| a.path.is_ident("command"))
            .map(|a| a.parse_meta().unwrap())
            .for_each(|meta| match meta {
                Meta::List(MetaList { nested, .. }) => nested.into_iter()
                    .for_each(|n| match n {
                        NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. })) => {
                            if path.is_ident("choice") {
                                variant.choice = Some(str);
                            }
                        }
                        NestedMeta::Meta(Meta::Path(path)) => {
                            if path.is_ident("default") {
                                variant.default = true;
                            }
                        }
                        _ => eprintln!("(enum) n = {:?}", n),
                    }),
                _ => eprintln!("(enum) meta = {:?}", meta),
            });

        Ok(variant)
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
            quote_spanned! { span => discorsd::model::interaction::CommandChoice::new(#name, #value) }
        });
        quote! {
            #(#choices),*
        }
    }

    fn branches(&self) -> TokenStream2 {
        let branches = self.0.iter().map(|v| {
            let str = v.name();
            let ident = &v.ident;
            quote_spanned! { v.ident.span() => #str => Ok(Self::#ident) }
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
                syn::Error::new(
                    ty.span(),
                    format!("Only one variant can be marked default (`{}` all are)", variants),
                ).into_compile_error()
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
