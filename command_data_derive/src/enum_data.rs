use std::convert::TryFrom;
use std::iter::FromIterator;

use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, quote_spanned};
use syn::{DataEnum, Fields, Ident, LitStr};
use syn::spanned::Spanned;

use crate::struct_data::Struct;

pub fn enum_impl(ty: &Ident, data: DataEnum) -> TokenStream2 {
    let variants: Result<Enum, syn::Error> = data.variants
        .into_iter()
        .map(Variant::try_from)
        .collect();
    let variants = match variants {
        Ok(s) => s,
        Err(err) => return err.into_compile_error(),
    };
    let match_branches = variants.match_branches(ty);
    let variants_array = variants.variants_array();

    let tokens = quote! {
        impl ::std::convert::TryFrom<ApplicationCommandInteractionData> for #ty {
            type Error = (discorsd::errors::CommandParseError, discorsd::model::ids::CommandId);

            fn try_from(data: ApplicationCommandInteractionData) -> ::std::result::Result<Self, Self::Error> {
                let name = data.name;
                let id = data.id;
                let mut options = data.options;

                if options.len() != 1 {
                    return Err((discorsd::errors::CommandParseError::NoSubtype(name), id));
                }

                let option = options.remove(0);
                let name = option.name;
                let options = option.options;
                match name.as_str() {
                    #match_branches
                    _ => Err((discorsd::errors::CommandParseError::UnknownOption(discorsd::errors::UnknownOption {
                        name,
                        options: &#variants_array,
                    }), id)),
                }
            }
        }
    };
    // eprintln!("tokens = {}", tokens.to_string());
    tokens
}

#[derive(Debug)]
struct Variant {
    ident: Ident,
    fields: Fields,
}

impl Variant {
    fn lowercase_name(&self) -> LitStr {
        LitStr::new(&self.ident.to_string().to_lowercase(), self.ident.span())
    }
}

impl TryFrom<syn::Variant> for Variant {
    type Error = syn::Error;

    fn try_from(variant: syn::Variant) -> Result<Self, Self::Error> {
        if variant.discriminant.is_some() {
            return Err(syn::Error::new(variant.span(), "Command variants can't have discriminants (ex, `= 1`)"));
        }

        Ok(Self { ident: variant.ident, fields: variant.fields })
    }
}

#[derive(Debug)]
struct Enum(Vec<Variant>);

impl Enum {
    fn match_branches(&self, ty: &Ident) -> TokenStream2 {
        let branches = self.0.iter().enumerate().map(|(n, v)| {
            let span = v.ident.span();
            let patt = v.lowercase_name();
            match Struct::from_fields(v.fields.clone()) {
                Ok(fields) => match syn::parse_str(&format!("{}::{}", ty, v.ident)) {
                    Ok(path) => {
                        let try_from_body = fields.try_from_body(ty, &path, n);
                        quote_spanned! { span =>
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

    fn variants_array(&self) -> TokenStream2 {
        let array = self.0.iter()
            .map(|v| LitStr::new(&v.ident.to_string().to_lowercase(), v.ident.span()));
        quote! { [#(#array),*] }
    }
}

impl FromIterator<Variant> for Enum {
    fn from_iter<T: IntoIterator<Item=Variant>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}