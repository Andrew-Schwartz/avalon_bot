use std::fmt::Display;

use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use syn::{GenericArgument, PathArguments, spanned::Spanned, Type};

pub fn command_data_impl(command_type: Option<&Type>) -> (TokenStream2, TokenStream2) {
    match command_type {
        None => {
            (quote! {
                impl<C: ::discorsd::commands::SlashCommand> ::discorsd::model::commands::CommandData<C>
            }, quote! {
                C
            })
        }
        Some(ident) => {
            (quote_spanned! { ident.span() =>
                impl ::discorsd::model::commands::CommandData<#ident>
            }, quote_spanned! { ident.span() =>
                #ident
            })
        }
    }
}

pub trait TypeExt {
    fn generic_type_by<F>(&self, pred: F) -> Option<&Type>
        where F: FnOnce(&Ident) -> bool;

    fn generic_type_of<I>(&self, ident: &I) -> Option<&Type>
        where I: ?Sized,
              Ident: PartialEq<I>, {
        self.generic_type_by(|i| i == ident)
    }

    fn generic_type(&self) -> Option<&Type> {
        self.generic_type_by(|_| true)
    }

    fn array_type(&self) -> Option<&Type>;

    fn without_generics(&self) -> Option<&Ident>;
}

impl TypeExt for Type {
    fn generic_type_by<F: FnOnce(&Ident) -> bool>(&self, pred: F) -> Option<&Type> {
        if let Type::Path(path) = self {
            if let Some(seg) = path.path.segments.first() {
                if pred(&seg.ident) {
                    if let PathArguments::AngleBracketed(args) = &seg.arguments {
                        if let ::std::prelude::v1::Some(GenericArgument::Type(ty)) = args.args.first() {
                            Some(ty)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn array_type(&self) -> Option<&Type> {
        if let Type::Array(array) = self {
            Some(&array.elem)
        } else {
            None
        }
    }

    fn without_generics(&self) -> Option<&Ident> {
        if let Type::Path(path) = self {
            path.path.segments.first().map(|seg| &seg.ident)
        } else {
            None
        }
    }
}

pub trait IteratorJoin {
    type Item;

    fn join(self, sep: &str) -> String where Self::Item: Display;
}

impl<T, I: Iterator<Item=T>> IteratorJoin for I {
    type Item = T;

    fn join(mut self, sep: &str) -> String where T: Display {
        // taken from Itertools::join
        match self.next() {
            None => String::new(),
            Some(first_elt) => {
                use std::fmt::Write;
                // estimate lower bound of capacity needed
                let (lower, _) = self.size_hint();
                let mut result = String::with_capacity(sep.len() * lower);
                write!(&mut result, "{}", first_elt).unwrap();
                for elt in self {
                    result.push_str(sep);
                    write!(&mut result, "{}", elt).unwrap();
                }
                result
            }
        }
    }
}