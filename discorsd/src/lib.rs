// todo
//! Woah!!! Discord library!

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
    // pedantic
    clippy::map_unwrap_or,
    // todo
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    // nursery
    clippy::missing_const_for_fn,
)]
// @formatter:on

#[macro_use]
extern crate bitflags;

/// Re-exported for use with [`Bot`](bot::Bot), [`SlashCommand`](commands::SlashCommand), etc.
pub use async_trait::async_trait;

pub use bot::*;
pub use cache::IdMap;

#[macro_use]
mod macros;
mod cache;
mod serde_utils;
mod utils;

pub mod bot;
pub mod commands;
pub mod errors;
pub mod http;
pub mod model;
pub mod shard;
