#![warn(clippy::pedantic, clippy::nursery)]
// @formatter:off
#![allow(
    clippy::similar_names,
    clippy::option_if_let_else,
    clippy::filter_map,
    clippy::use_self,
    clippy::default_trait_access,
    // pedantic
    clippy::wildcard_imports,
    clippy::too_many_lines,
)]
// @formatter:on

use proc_macro::TokenStream;

use proc_macro2::{Ident, Span};
use proc_macro_error::*;
use quote::quote;
use syn::{Data, DeriveInput, parse_macro_input};

#[macro_use]
mod macros;
pub(crate) mod utils;
mod struct_data;
mod enum_data;
mod enum_option;

/// Todo document these
/// asdasdhasdha
#[proc_macro_derive(CommandData, attributes(command))]
#[proc_macro_error]
pub fn derive_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ty = input.ident;
    dummy_impl(&ty);

    let tokens = match input.data {
        Data::Struct(data) => struct_data::struct_impl(&ty, data.fields, &input.attrs),
        Data::Enum(data) => enum_data::enum_impl(&ty, data, &input.attrs),
        Data::Union(_) => abort!(ty, "Can't derive `CommandData` on a Union"),
    };

    tokens.into()
}

#[proc_macro_derive(CommandDataOption, attributes(command))]
#[proc_macro_error]
pub fn derive_option(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ty = input.ident;
    dummy_impl(&ty);

    let tokens = match input.data {
        Data::Struct(_) => abort!(
            ty,
            "Can't derive `CommandDataOption` on a Struct (yet?)",
        ),
        Data::Enum(data) => enum_option::enum_impl(&ty, data),
        Data::Union(_) => abort!(
            ty,
            "Can't derive `CommandDataOption` on a Union",
        ),
    };

    tokens.into()
}

fn dummy_impl(ty: &Ident) {
    let fail_enum = Ident::new(&format!("{}DeriveFailed", ty), Span::call_site());
    set_dummy(quote! {
        enum #fail_enum {}
        impl ::discorsd::commands::OptionsLadder for #fail_enum {
            type Raise = Self;
            type Lower = Self;
            fn from_data_option(
                _: ::discorsd::commands::InteractionDataOption
            ) -> ::std::result::Result<Self, ::discorsd::errors::CommandParseError> {
                unimplemented!()
            }
        }
        impl ::discorsd::commands::VecArgLadder for #fail_enum {
            type Raise = Self;
            type Lower = Self;

            fn tlo_ctor() -> fn(::std::vec::Vec<Self>) -> ::discorsd::commands::TopLevelOption {
                unimplemented!()
            }

            fn make(_: &'static str, _: &'static str, _: ::std::vec::Vec<Self::Lower>) -> Self {
                unimplemented!()
            }
        }

        impl<C: ::discorsd::commands::SlashCommand> ::discorsd::model::commands::CommandData<C> for #ty {
            type Options = #fail_enum;
            fn from_options(_: Self::Options) -> ::std::result::Result<Self, ::discorsd::errors::CommandParseError> {
                unimplemented!()
            }
            type VecArg = #fail_enum;
            fn make_args(_: &C) -> ::std::vec::Vec<Self::VecArg> {
                unimplemented!()
            }
        }
    });
}