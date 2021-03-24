#![warn(clippy::pedantic, clippy::nursery)]
// @formatter:off
#![allow(
    clippy::similar_names,
    clippy::option_if_let_else,
    clippy::filter_map,
    clippy::use_self,
    clippy::default_trait_access,
)]
// @formatter:on

use proc_macro::TokenStream;

use syn::{DeriveInput, parse_macro_input, Data};

pub(crate) mod utils;
mod struct_data;
mod enum_option;
mod enum_data;

/// Todo document these
#[proc_macro_derive(CommandData, attributes(command))]
pub fn derive_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let ty = input.ident;
    let tokens = match input.data {
        Data::Struct(data) => struct_data::struct_impl(&ty, data.fields, &input.attrs),
        Data::Enum(data) => enum_data::enum_impl(&ty, data),
        Data::Union(_) => syn::Error::new(
            ty.span(),
            "Can't derive `CommandData` on a Union",
        ).into_compile_error(),
    };

    tokens.into()
}

#[proc_macro_derive(CommandDataOption, attributes(command))]
pub fn derive_option(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ty = input.ident;
    let tokens = match input.data {
        Data::Struct(_) => syn::Error::new(
            ty.span(),
            "Can't derive `CommandDataOption` on a Struct (yet?)",
        ).into_compile_error(),
        Data::Enum(data) => enum_option::enum_impl(&ty, data),
        Data::Union(_) => syn::Error::new(
            ty.span(),
            "Can't derive `CommandDataOption` on a Union",
        ).into_compile_error(),
    };

    tokens.into()
}