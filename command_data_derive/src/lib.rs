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
use enum_data::{Enum as DataEnum, Variant as DataVariant};
use struct_data::*;

use crate::utils::TypeExt;

#[macro_use]
mod macros;
pub(crate) mod utils;
mod struct_data;
mod enum_data;
mod enum_choices;

/// See Documentation macros
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

/// See Documentation macros
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
handle_attribute!(
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
        // todo is this necessary? it's never used
        /// Make this field required (note: fields are required by default, unless they are an `Option`).
        ["required" => self.default = None]
        /// Only applicable for `vararg` fields. Name the command options "One", "Two", "Three", etc.
        ["va_ordinals" => self.vararg.names = VarargNames::Ordinals]
        /// Only applicable for `vararg` fields. Name this vararg field "{vararg}1", "{vararg}2",
        /// where {vararg} is the key on the `vararg` option.
        ///
        /// Note: this is the default naming behavior used for varargs.
        ["va_indexed" => self.vararg.names = VarargNames::Index],

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
        ["vararg" => self.vararg.root = Some(str)]
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
        ["va_count" => self.vararg.num = VarargNum::Function(str.parse()?)]
        /// How to name the vararg options. Must be callable as a function
        /// `fn<N>(usize) -> N where N: Into<Cow<'static, str>`.
        ["va_names" => self.vararg.names = VarargNames::Function(str.parse()?)],

    " = {int}": Meta::NameValue(MetaNameValue { path, lit: Lit::Int(int), .. }), path =>
        /// The number of vararg options to show.
        ["va_count" => self.vararg.num = VarargNum::Count(int.base10_parse()?)]
        /// The number of vararg options required. If `va_count` is greater than this, the excess
        /// options will be optional.
        ["required" => self.vararg.required = if self.ty.array_type().is_some() {
            // if its an array require all of them
            None
        } else {
            Some(int.base10_parse()?)
        }]
);

handle_attribute!(
    /// Attributes on a struct, for example `type` on `MyData`:
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
        ["command" => self.command_type = Some(str.parse()?)]
);

handle_attribute!(
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
    self: DataVariant =>
    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// The description of this command option.
        ["desc" => self.desc = Some(str)]
        /// What to rename this field as in the Command.
        ["rename" => self.rename = Some(str)]
);

handle_attribute!(
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
    self: DataEnum =>
    " = {str}": Meta::NameValue(MetaNameValue { path, lit: Lit::Str(str), .. }), path =>
        /// Specify the type of the `SlashCommand` that this is data for. Useful for annotations that
        /// can make decisions at runtime by taking functions callable as `fn(CommandType) -> SomeType`.
        ["command" => self.command_type = Some(str.parse()?)]
);

handle_attribute!(
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
        ["choice" => self.choice = Some(str)]
);