#![warn(clippy::pedantic, clippy::nursery)]
// @formatter:off
#![allow(
    clippy::module_name_repetitions,
    clippy::struct_excessive_bools,
    clippy::wildcard_imports,
    clippy::enum_glob_use,
    clippy::default_trait_access,
    clippy::option_option,
    clippy::empty_enum,
    clippy::match_same_arms,
    clippy::must_use_candidate,
    clippy::option_if_let_else,
    clippy::manual_non_exhaustive,
    // todo
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    // nursery
    clippy::missing_const_for_fn,
)]
// @formatter:on

#[macro_use]
extern crate bitflags;

pub use async_trait::async_trait;

pub use bot::*;
pub use cache::IdMap;
pub use markup_extensions::*;

#[macro_use]
mod macros;
mod cache;

pub mod http;
pub mod shard;
pub mod serde_utils;
pub mod bot;
pub mod utils;
pub mod markup_extensions;
pub mod errors;
pub mod model;
pub mod commands;
