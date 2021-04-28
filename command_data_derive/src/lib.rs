//! Derive macros which allow for the easy creation of Discord Slash Commands. Simply create a
//! struct or enum that contains the data you want the user of the command to fill out, derive
//! `CommandData`, and annotate the fields or variants as needed to customize the command.
//!
//! # Simple Example
//! Starting simply, we can imagine a command that would pick a number between upper and lower
//! limits.
//! ```rust
//! # /*
//! #[derive(CommandData)]
//! # */
//! struct RandomNumberData {
//!     # /*
//!     #[command(desc = "The lower limit of the random number range")]
//!     # */
//!     lower: i64,
//!     # /*
//!     #[command(desc = "The upper limit of the random number range")]
//!     # */
//!     upper: i64,
//! }
//! ```
//! This will create a command that like such (the command's name & description is set elsewhere):
//! ![random command](https://github.com/Andrew-Schwartz/avalon_bot/blob/master/images/docs/random_number.png?raw=true)
//! ![random command arg](https://github.com/Andrew-Schwartz/avalon_bot/blob/master/images/docs/random_number2.png?raw=true)
//!
//! # Discord's `/permissions` Command
//! For a more complex example, we will create the example `/permissions` the Discord docs use to
//! show off the structure of a Slash Command
//! [here](https://discord.com/developers/docs/interactions/slash-commands#example-walkthrough)
//! using this macro. This `/permission` command has two subgroups, one that deals with user
//! permissions, the other deals with role permissions. Each of these groups has two subcommands,
//! one to get permissions, one to edit permissions. Each of these subcommands takes a user/role id
//! and an optional channel id argument. The json shown in Discord's docs is quite complex, but the
//! same command can be created with the following Rust code:
//! ```rust
//! # struct UserId; struct ChannelId; struct RoleId;
//! # /*
//! #[derive(CommandData)]
//! # */
//! enum PermissionsData {
//! #     /*
//!     #[command(desc = "Get or edit permissions for a user")]
//!     # */
//!     User(GetEditUser),
//!     # /*
//!     #[command(desc = "Get or edit permissions for a role")]
//!     # */
//!     Role(GetEditRole),
//!}
//! # /*
//! #[derive(CommandData)]
//! # */
//! enum GetEditUser {
//!     # /*
//!     #[command(desc = "Get permissions for a user")]
//!     # */
//!     Get {
//!         # /*
//!         #[command(desc = "The user to get")]
//!         # */
//!         user: UserId,
//!         # /*
//!         #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
//!         # */
//!         channel: Option<ChannelId>,
//!     },
//!     # /*
//!     #[command(desc = "Edit permissions for a user")]
//!     # */
//!     Edit {
//!         # /*
//!         #[command(desc = "The user to edit")]
//!         # */
//!         user: UserId,
//!         # /*
//!         #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
//!         # */
//!         channel: Option<ChannelId>,
//!     },
//! }
//! # /*
//! #[derive(CommandData)]
//! # */
//! enum GetEditRole {
//!     # /*
//!     #[command(desc = "Get permissions for a role")]
//!     # */
//!     Get(GetRole),
//!     # /*
//!     #[command(desc = "Edit permissions for a role")]
//!     # */
//!     Edit(EditRole),
//! }
//! # /*
//! #[derive(CommandData)]
//! # */
//! struct GetRole {
//!     # /*
//!     #[command(desc = "The role to get")]
//!     # */
//!     role: RoleId,
//!     # /*
//!     #[command(desc = "The channel permissions to get. If omitted, the guild permissions will be returned")]
//!     # */
//!     channel: Option<ChannelId>,
//! }
//! # /*
//! #[derive(CommandData)]
//! # */
//! struct EditRole {
//!     # /*
//!     #[command(desc = "The role to edit")]
//!     # */
//!     role: RoleId,
//!     # /*
//!     #[command(desc = "The channel permissions to edit. If omitted, the guild permissions will be edited")]
//!     # */
//!     channel: Option<ChannelId>,
//! }
//! ```
//! The output is the same as shown in Discord's documentation,
//! ![permissions command](https://discord.com/assets/5002338abeaf8532983a1be594a10683.png)
//!
//! The impl of `CommandData` for `PermissionsData` in the above example, produces *exactly* the
//! same json when serialized as in Discord's documentation, while also provinding type-safe
//! facilites to deserialize received Slash Command invocations into the `PermissionsData` enum.
//!
//! Note that the `GetEditUser` enum, which contains inline struct definitions of the `Get` and
//! `Edit` data, is equivalent to the `GetEditRole` enum (with the exception of `UserId` ->
//! `RoleId`), which simply wraps each of it's separate data structs in newtype variants. What this
//! means, is that if you want to create a Slash Command that is just one static command, the data
//! should be in a `struct`. If you want to create a Slash Command that consists of multiple
//! subcommands, the main data should be in an `enum` which either declares the command data structs
//! inline or which wraps data structs in a one element tuple (these styles can't be mixed within
//! one enum), or unit variants. Finally, to create a Slash Command that consists of subgroups, the
//! main data should be an `enum` with single element tuple variants that each wrap an enum that
//! fits the above requirements to be subcommand data.
//!
//! In addition, you can see that, by making a field's type `Option<T>`, that option will not be
//! required in the use of the Slash Command.
//!
//! # Vararg Commands
//! An additional feature the `CommandData` derive macro supports is taking a variable amount of
//! parameters (varargs) as options in your command. Various collection types from the standard
//! library can be used for vararg parameters. These include `Vec<T>`, `HashSet<T>`, `BTreeSet<T>`,
//! and `[T; N]`, where `T` itself implements `CommandData` as a single option as well as any other
//! traits needed to use a collection of that type (`Hash`, `Eq`, etc). For the array case, `N` is
//! any `usize` up to 25, the maximum number of options a Discord command. For an array, there will
//! always be `N` repeated options; for the other types, the number of required and total options
//! (ie, including optional options) can be configured to dynamically update at runtime.
//!
//! For example, we'll make a command that will ban or unban users, always requiring one user to
//! (un)ban and allowing up to 5 total users to be (un)banned.
//! ```rust
//! # struct UserId;
//! # /*
//! #[derive(CommandData)]
//! # */
//! enum BanUnbanData {
//!     # /*
//!     #[command(desc = "Ban user(s)")]
//!     # */
//!     Ban(
//!         # /*
//!         #[command(vararg = "user", va_req = 1, va_count = 5)]
//!         # */
//!         Vec<UserId>,
//!     ),
//!     # /*
//!     #[command(desc = "Unban user(s)")]
//!     # */
//!     Unban(
//!         # /*
//!         #[command(vararg = "user", va_req = 1, va_count = 5)]
//!         # */
//!         Vec<UserId>,
//!     ),
//! }
//! ```
//! ![ban unban command](https://github.com/Andrew-Schwartz/avalon_bot/blob/master/images/docs/ban_unban.png?raw=true)
//!
//! Multiple vararg options can be used in a row, as long as any optional arguments are at the end
//! due some of Discord's Slash Command limits. For example,
//! ```rust
//! # /*
//! #[derive(CommandData)]
//! # */
//! pub struct TestData(
//! # /*
//!     #[command(vararg = "num")]
//! # */
//!     [i64; 3],
//! # /*
//!     #[command(vararg = "number", va_count = 3, va_req = 1)]
//! # */
//!     Vec<i64>,
//! );
//! ```
//! works, but
//! ```rust
//! # /*
//! #[derive(CommandData)]
//! # */
//! pub struct TestData(
//! # /*
//!     #[command(vararg = "number", va_count = 3, va_req = 1)]
//! # */
//!     Vec<i64>,
//! # /*
//!     #[command(vararg = "num")]
//! # */
//!     [i64; 3],
//! );
//! ```
//! doesn't.
//!
//! Varargs can be configured in various ways as documented in
//! [`Documentation_For_Field`](Documentation_For_Field!).
//!
//! # Choices
//! One nice feature of Discord's Slash Commands is the ability to set all of the possible choices
//! usable for a command option. For example, we could have a command to get some information about
//! the device running our bot, such as the CPU usage, memory usage, and component temperatures.
//! Because this crate emphasizes the importance of type safety, the ability to use an data-less
//! enum to generate these choices is supported with the `CommandDataChoices` derive.
//!
//! ```rust
//! # /*
//! #[derive(CommandDataChoices)]
//! # */
//! enum Information {
//!     # /*
//!     #[command(default, choice = "All Information")]
//!     # */
//!     All,
//!     Cpu,
//!     Memory,
//!     Temperature,
//! }
//! # /*
//! #[derive(CommandData)]
//! # */
//! struct InfoData {
//!     # /*
//!     #[command(desc = "The type of data to get", default)]
//!     # */
//!     data: Information,
//! }
//! ```
//! ![info command](https://github.com/Andrew-Schwartz/avalon_bot/blob/master/images/docs/info.png?raw=true)
//!
//! A command using the above `InfoData` will consist of a single optional parameter called `data`,
//! which will default to `Information::All` if not entered by the user.
//! See [`Documentation_For_ChoicesVariant`](Documentation_For_ChoicesVariant!) for more information on
//! attributes which can be applied to the choices.

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
// for handle_attributes!
use syn::{Lit, Meta, MetaList, MetaNameValue, NestedMeta};

use enum_choices::Variant as ChoicesVariant;
use enum_data::{Enum, Variant};
use struct_data::*;

use crate::utils::TypeExt;

#[macro_use]
mod macros;
pub(crate) mod utils;
mod struct_data;
mod enum_data;
mod enum_choices;

/// See crate level documentation for general info, and the
/// [Documentation_For_Field](Documentation_For_Field!),
/// [Documentation_For_Struct](Documentation_For_Struct!),
/// [Documentation_For_Variant](Documentation_For_Variant!),
/// and [Documentation_For_Enum](Documentation_For_Enum!) macros for details on all
/// `#[command(...)]` attributes.
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

/// See crate level documentation for general info, and the
/// [Documentation_For_ChoicesVariant](Documentation_For_ChoicesVariant!) macro for details on all
/// `#[command(...)]` attributes.
#[proc_macro_derive(CommandDataChoices, attributes(command))]
#[proc_macro_error]
pub fn derive_option(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ty = input.ident;
    dummy_impl(&ty);

    let tokens = match input.data {
        Data::Struct(_) => abort!(
            ty,
            "Can't derive `CommandDataChoices` on a Struct (yet?)",
        ),
        Data::Enum(data) => enum_choices::enum_impl(&ty, data),
        Data::Union(_) => abort!(
            ty,
            "Can't derive `CommandDataChoices` on a Union",
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

        impl<C: ::discorsd::commands::SlashCommandRaw> ::discorsd::model::commands::CommandData<C> for #ty {
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

// handle_attributes! invoked here to generate documentation
handle_attribute! {
    /// Attributes on a struct field, for example `desc` on `MyData.user`:
    /// ```
    /// # struct UserId;
    /// # const IGNORE: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// pub struct MyData {
    /// # #[doc = r#"
    ///     #[command(desc = "Pick a user")]
    /// # "#]
    ///     user: UserId,
    /// }
    /// ```
    self: Field =>

    "": Meta::Path(path), path =>
        /// Marks this field as optional in the Command in Discord, and if the user omits it, will use
        /// this field's type's `Default` implementation to create it.
        ["default" => self.default = Some(syn::parse_str("::std::default::Default::default").unwrap())]
        // // todo is this necessary? it's never used
        // /// Make this field required (note: fields are required by default, unless they are an `Option`).
        // ["required" => self.default = None]
        /// Makes this field a `vararg`. Names the command options "One", "Two", "Three", etc.
        ["va_ordinals" => self.vararg.get_or_insert_with(Default::default).names = VarargNames::Ordinals],

    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// The description of this command option. If omitted, will use the field's name as the
        /// description.
        ["desc" => self.desc = Some(str)]
        /// Marks this field as optional in the Command in Discord, and if the user omits it, will use
        /// this function to provide the default if this field is missing. Must be callable as
        /// `fn() -> T`, where `T` is this field's type.
        ["default" => self.default = Some(str.parse()?)]
        /// What to rename this field as in the Command.
        ["rename" => {
            if let FieldIdent::Named(named) = &mut self.name {
                named.rename = Some(str);
            }
        }]
        // todo make a va_desc_name thing
        /// Marks this field as a vararg argument to the command, with the name and description
        /// created by appending a counting integer to `{str}`. Allows the user to chose multiple
        /// options of this field's type.
        /// See also `ordinals`, `counts`, `va_count`, and `va_names`.
        ["vararg" => self.vararg.get_or_insert_with(Default::default).names = VarargNames::Index(str)]
        // todo reorder doc so the type = ... part comes first cuz its basically always going to be
        //  set if you're doing this
        /// How to filter the choices, if `choices` is true.
        ///
        /// Must be a function callable as
        /// `fn<C: SlashCommand>(&C, &CommandChoice<&'static str>) -> bool`
        /// if the type for this data is not set, or as
        /// `fn(&Command, &CommandChoice<&'static str) -> bool`
        /// where `Command` is the right hand side of `#[command(command = ...)]` on the struct if it is.
        ["retain" => self.retain = Some(str.parse()?)]
        /// Function to determine if this field is required, must be callable as
        /// `fn<C: SlashCommand>(&C) -> bool`, where the generic is not necessary if the
        /// struct's type is specified (with `#[command(command = "MyCommand")]` as above).
        ["required" => self.required = Some(str.parse()?)]
        /// `fn<C: SlashCommand>(&C) -> usize` to pick how many vararg options to display.
        /// The the same generic rules apply as above. If you want a fixed number of varargs in the
        /// command, set `required` to an int.
        ["va_count" => self.vararg.get_or_insert_with(Default::default).num = VarargNum::Function(str.parse()?)]
        /// How to name the vararg options. Must be callable as a function
        /// `fn(usize) -> N`, where `N: Into<Cow<'static, str>`.
        ["va_names" => self.vararg.get_or_insert_with(Default::default).names = VarargNames::Function(str.parse()?)],

    " = {int}": Meta::NameValue(MetaNameValue { path, lit: Lit::Int(int), .. }), path =>
        /// The number of vararg options to show.
        ["va_count" => self.vararg.get_or_insert_with(Default::default).num = VarargNum::Count(int.base10_parse()?)]
        /// The number of vararg options required. If `va_count` is greater than this, the excess
        /// options will be optional.
        ["va_req" => self.vararg.get_or_insert_with(Default::default).required = if self.ty.array_type().is_some() {
            // if its an array require all of them
            None
        } else {
            Some(int.base10_parse()?)
        }],
}

handle_attribute! {
    /// Attributes on a struct, for example `command` on `MyData`:
    /// ```
    /// # struct UserId;
    /// # trait SlashCommand { const IGNORE: &'static str; }
    /// #[derive(Debug, Clone)]
    /// struct MyCommand;
    /// impl SlashCommand for MyCommand {
    /// #   const IGNORE: &'static str = stringify!(
    ///     ...
    /// #   );
    /// }
    ///
    /// # const IGNORE: &str = stringify!(
    /// #[derive(CommandData)]
    /// #[command(command = "MyCommand")]
    /// # );
    /// pub struct MyData {
    ///     user: UserId,
    /// }
    /// ```
    self: Struct =>

    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// Specify the type of the `SlashCommand` that this is data for. Useful for annotations that
        /// can make decisions at runtime by taking functions callable as `fn(&CommandType) -> SomeType`.
        ["command" => self.command_type = Some(str.parse()?)],
}

handle_attribute! {
    /// Attributes on a Data enum variant, for example `desc` on `MyData::Add`:
    /// ```
    /// # struct UserId;
    /// # const IGNORE: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// pub enum MyData {
    /// # #[doc = r#"
    ///     #[command(desc = "Pick a user")]
    /// # "#]
    ///     Add(UserId),
    ///     Remove(UserId),
    ///     Clear,
    /// }
    /// ```
    self: Variant =>
    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// The description of this command option.
        ["desc" => self.desc = Some(str)]
        /// What to rename this field as in the Command.
        ["rename" => self.rename = Some(str)]
        /// The path to a function callable as `fn(&CommandType) -> bool` to determine whether to
        /// enable this variant's option in Discord.
        ["enable_if" => self.enable_if = Some(str.parse()?)],
}

handle_attribute! {
    /// Attributes on a Data enum variant, for example `desc` on `MyData::Add`:
    /// ```
    /// # struct UserId;
    /// # trait SlashCommand { const IGNORE: &'static str; }
    /// #[derive(Debug, Clone)]
    /// struct MyCommand;
    /// impl SlashCommand for MyCommand {
    /// # const IGNORE: &'static str = stringify!(
    ///     ...
    /// # );
    /// }
    ///
    /// # const IGNORE: &str = stringify!(
    /// #[derive(CommandData)]
    /// #[command(command = "MyCommand")]
    /// # );
    /// pub enum MyData {
    ///     Add(UserId),
    ///     Remove(UserId),
    ///     Clear,
    /// }
    /// ```
    ///
    /// All variants will be shown as lowercase in Discord.
    self: Enum =>
    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// Specify the type of the `SlashCommand` that this is data for. Useful for annotations that
        /// can make decisions at runtime by taking functions callable as `fn(CommandType) -> SomeType`.
        ["command" => self.command_type = Some(str.parse()?)],
}

handle_attribute! {
    /// Attributes on a Choices enum variant, for example `default` on `MyData::OptionB`:
    /// ```
    /// # const IGNORE: &str = stringify!(
    /// #[derive(CommandDataChoices)]
    /// # );
    /// pub enum MyData {
    ///     OptionA,
    /// # #[doc = r#"
    ///     #[command(default)]
    /// # "#]
    ///     OptionB,
    ///     OptionC,
    /// }
    /// ```
    /// All variants must be unit structs.
    ///
    self: ChoicesVariant =>
    "": Meta::Path(path), path =>
        /// Implement `Default` for this enum, with this field as the default.
        ["default" => self.default = true],

    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// The string to show in Discord for this choice. Useful when you want to display a multiple
        /// word long choice.
        ["choice" => self.choice = Some(str)],
}