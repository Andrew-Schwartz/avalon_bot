use std::fmt::Display;
use std::ops::Not;

use proc_macro2::{Ident, TokenStream as TokenStream2};
use quote::{quote, quote_spanned};
use syn::{GenericArgument, PathArguments, spanned::Spanned, Type, TypeParam};

/// Generics including type bounds
pub fn declaration_generics<I>(generics: &[TypeParam], additional_bounds: I) -> TokenStream2
    where I: Iterator<Item=TokenStream2>
{
    let additional_bounds = quote! { #(#additional_bounds)+* };
    let generics = generics.iter().map(|g| {
        let ident = &g.ident;
        let bounds = &g.bounds;
        let plus = bounds.is_empty().not().then(|| quote! { + });
        quote_spanned! { g.span() =>
            #ident: #bounds #plus #additional_bounds
        }
    });
    quote! { #(#generics,)* }
}

/// Generics without type bounds, no angle brackets
pub fn use_generics(generics: &[TypeParam]) -> TokenStream2 {
    let generics = generics.iter().map(|g| {
        let ident = &g.ident;
        quote_spanned! { g.span() => #ident }
    });
    quote! { #(#generics,)* }
}

pub fn replace_generics(format_string: &mut String, generics: &[TypeParam]) -> Option<TokenStream2> {
    let mut generics_used = Vec::new();
    for TypeParam { ident, .. } in generics {
        let pattern = format!("<{}>", ident);
        if let Some(idx) = format_string.find(&pattern) {
            format_string.replace_range(idx..idx + pattern.len(), "{}");
            generics_used.push(ident);
        }
    }
    generics_used.is_empty().not()
        .then(|| quote! { format!(#format_string #(, #generics_used::ARG_NAME)*) })
}

/// returns (impl statement, command type)
pub fn command_data_impl(command_type: Option<&Type>, generics: &[TypeParam]) -> (TokenStream2, TokenStream2) {
    let ty = match command_type {
        None => quote! { C },
        Some(ident) => quote_spanned! { ident.span() => #ident },
    };
    let generics = generics.iter().map(|g| {
        let ident = &g.ident;
        let bounds = &g.bounds;
        let plus = bounds.is_empty().not().then(|| quote! { + });
        quote_spanned! { g.span() =>
            #ident: #bounds
                #plus ::discorsd::commands::OptionCtor
                + ::discorsd::commands::CommandData<
                        #ty,
                        Options=::discorsd::commands::ValueOption,
                        Choice=#ident
                   >
        }
    });
    let impl_statement = match command_type {
        None => quote! { impl<C: ::discorsd::commands::SlashCommandRaw, #(#generics),*> ::discorsd::commands::CommandData<C> },
        Some(ident) => quote_spanned! { ident.span() => impl<#(#generics),*> ::discorsd::commands::CommandData<#ident> },
    };
    (impl_statement, ty)
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
            let seg = path.path.segments.first()?;
            if !pred(&seg.ident) { return None; }
            if let PathArguments::AngleBracketed(args) = &seg.arguments {
                if let Some(GenericArgument::Type(ty)) = args.args.first() {
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