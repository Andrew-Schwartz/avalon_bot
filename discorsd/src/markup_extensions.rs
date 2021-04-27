use std::fmt;

use crate::model::emoji::{CustomEmoji, Emoji};
use crate::model::ids::*;

// pub mod macros {
//     macro_rules! make_markup_macro {
//         ($name: ident, $surround: literal) => {
//             #[macro_export]
//             macro_rules! $ident {
//                 ($str: tt) => { format!(concat!($surround, "{}", $surround), $str) };
//             }
//         };
//     }
//
//     #[macro_export]
//     macro_rules! underline {
//         ($str: tt) => { format!("__{}__", $str) };
//     }
//     #[macro_export]
//     macro_rules! inline_code {
//         ($str: tt) => { format!("`{}`", $str) };
//     }
//     #[macro_export]
//     macro_rules! code_block {
//         ($lang:tt, $str: tt) => { format!("```{}{}```", $lang, $str) };
//         ($str: tt) => { format!("```{}```", $str) };
//     }
// }

// pub trait MarkupExt: AsRef<str> {
//     fn underline(&self) -> String {
//         format!("__{}__", self.as_ref())
//     }
// }
//
// impl<S: AsRef<str>> MarkupExt for S {}

// todo these can be elsewhere, then delete this file
