use std::iter::FromIterator;

use proc_macro2::TokenStream as TokenStream2;
use proc_macro_error::*;
use quote::{quote, quote_spanned};
use syn::{Attribute, DataEnum, Fields, Ident, LitStr, Path, Type, TypeParam};
use syn::spanned::Spanned;

use crate::struct_data::{description_len_check, Field, Struct};
use crate::utils::{command_data_impl, use_generics};

pub fn enum_impl(ty: &Ident, generics: Vec<TypeParam>, data: DataEnum, attrs: &[Attribute]) -> TokenStream2 {
    let mut variants: Enum = data.variants
        .into_iter()
        .map(Variant::from)
        .collect();
    variants.generics = generics;
    for attr in attrs {
        if !attr.path.is_ident("command") { continue; };
        variants.handle_attribute(attr);
    }
    variants.args_maker_impl(ty)
}

#[derive(Debug)]
pub struct Variant {
    attrs: Vec<Attribute>,
    ident: Ident,
    pub rename: Option<LitStr>,
    fields: Fields,
    pub desc: Option<LitStr>,
    /// fn<C>(c: &C) -> bool
    pub enable_if: Option<Path>,
}

impl Variant {
    // todo this might have to be able to handle generics
    fn name(&self) -> String {
        if let Some(lit) = &self.rename {
            lit.value().to_lowercase()
        } else {
            self.ident.to_string().to_lowercase()
        }
    }

    fn description(&self, name: &str, generics: &[TypeParam]) -> TokenStream2 {
        description_len_check(&self.desc, generics).unwrap_or_else(|| quote! { #name })
    }
}

impl From<syn::Variant> for Variant {
    fn from(variant: syn::Variant) -> Self {
        if variant.discriminant.is_some() {
            abort!(variant, "Command variants can't have discriminants (ex, `= 1`)");
        }
        let attrs = variant.attrs;
        let mut variant = Self {
            attrs: Vec::new(),
            ident: variant.ident,
            rename: None,
            fields: variant.fields,
            desc: None,
            enable_if: None,
        };
        for attr in &attrs {
            if !attr.path.is_ident("command") { continue; }
            variant.handle_attribute(attr);
        }
        variant.attrs = attrs;

        variant
    }
}

#[derive(Debug)]
pub struct Enum {
    variants: Vec<Variant>,
    /// settable with `#[command(type = MyCommand)]` on an enum
    pub command_type: Option<Type>,
    generics: Vec<TypeParam>,
}

impl Enum {
    //noinspection RsSelfConvention
    fn from_options_branches(&self, ty: &Ident, command_ty: &TokenStream2) -> TokenStream2 {
        let branches = self.variants.iter().map(|v| {
            // todo filter out the attributes this used (might not be a thing)
            let fields = Struct::from_fields(v.fields.clone(), &[], self.generics.clone());
            let patt = v.name();
            match syn::parse_str(&format!("{}::{}", ty, v.ident)) {
                Ok(path) => {
                    let ty = quote_spanned! { ty.span() => #ty };
                    let try_from_body = fields.impl_from_options(&ty, &path, command_ty);
                    quote_spanned! { v.ident.span() =>
                        #patt => {
                            #try_from_body
                        }
                    }
                }
                Err(e) => abort!(e),
            }
        });
        quote! {
            #(#branches,)*
        }
    }

    fn make_args_vec(&self, command_type: &TokenStream2) -> TokenStream2 {
        let chains = self.variants.iter().map(|v| {
            // todo filter out the attributes this used (might not be a thing)
            let strukt = Struct::from_fields(v.fields.clone(), &[], self.generics.clone());
            let name = v.name();
            let desc = v.description(&name, &self.generics);
            let options = strukt.data_options(command_type);
            let take = if let Some(enable) = &v.enable_if {
                quote_spanned! { enable.span() =>
                    .take(#enable(command) as usize)
                }
            } else {
                TokenStream2::new()
            };
            quote_spanned! { v.ident.span() =>
                ::std::iter::once(<Self::VecArg as ::discorsd::commands::VecArgLadder>::make(
                    #name, #desc, #options
                ))#take
            }
        });

        quote! {
            ::std::iter::empty()
                #(.chain(#chains))*
                .collect()
        }
    }

    fn variants_array(&self) -> TokenStream2 {
        let array = self.variants.iter().map(Variant::name);
        quote! { [#(#array),*] }
    }

    fn args_maker_impl(&self, ty: &Ident) -> TokenStream2 {
        fn is_inline(variant: &Variant) -> Option<bool> {
            match &variant.fields {
                Fields::Named(_) => Some(true),
                Fields::Unnamed(fields) => {
                    match fields.unnamed.len() {
                        0 => None,
                        1 => {
                            let field = fields.unnamed.first().unwrap();
                            let field = Field::from((0, field.clone()));
                            if field.vararg.is_some() {
                                Some(true)
                            } else {
                                Some(false)
                            }
                        }
                        _ => Some(true),
                    }
                }
                Fields::Unit => None,
            }
        }
        let differ_err = |variant: &Variant| abort!(
            variant.fields,
            "All variants must be same type (tuple/struct), but this one isn't",
        );

        // None        - No variants
        // Some(true)  - All (some) variants are inline structs, ie `Variant { name: type },`
        //               This enum is a list of subcommand
        // Some(false) - All are one field tuples, ie `Variant(Type)`
        //               This enum is a list of command groups OR subcommands, depending on how
        //               `Type` implements `CommandData`
        let mut inline_data = None::<bool>;
        for variant in &self.variants {
            if let Some(inline_data) = inline_data {
                // make sure each variant is same type
                match is_inline(variant) {
                    Some(true) => {
                        if !inline_data {
                            differ_err(variant);
                        }
                    }
                    Some(false) => {
                        if inline_data {
                            differ_err(variant);
                        }
                    }
                    // just skip unit structs
                    None => {}
                }
            } else {
                // first variant, set `inline_data`
                if let Some(is_inline) = is_inline(variant) {
                    inline_data = Some(is_inline);
                }
            }
        }

        match inline_data {
            None => abort_call_site!("Empty enums can't be Command Data"),
            Some(true) => self.inline_structs(ty),
            Some(false) => self.newtype_structs(ty),
        }
    }

    /// Enums where each variant is a newtype
    /// ```
    /// # const IGNORE1: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// struct Color { hex: String }
    /// # const IGNORE2: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// struct Person { name: String, age: u32 }
    ///
    /// # const IGNORE3: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// enum Data {
    ///     ColorCommand(String),
    ///     PersonCommand(Person),
    /// #   /*
    ///     ...
    /// #   */
    /// }
    /// ```
    /// This also works if the inner of the newtype is an enum, as long as you `#[derive(CommandData)]`
    fn newtype_structs(&self, ty: &Ident) -> TokenStream2 {
        let generic_ty = {
            let use_generics = use_generics(&self.generics);
            quote! { #ty<#use_generics> }
        };
        let (command_data_impl, c_ty) = command_data_impl(self.command_type.as_ref(), &self.generics);
        let first_variant_ty = &self.variants.iter()
            .find(|v| !matches!(&v.fields, Fields::Unit))
            .expect("Enum is not empty")
            .fields.iter()
            .next()
            .expect("All newtype enums have at least one newtype")
            .ty;
        let args = self.variants.iter().map(|v| {
            let name = v.name();
            let desc = v.description(&name, &self.generics);
            let make_args = if let Some(f) = &v.fields.iter().next() {
                let new_ty = &f.ty;
                quote_spanned! { new_ty.span() => <#new_ty>::make_args(command) }
            } else {
                quote! { Vec::new() }
            };
            let quote = quote_spanned! { v.ident.span() =>
                <Self::VecArg as ::discorsd::commands::VecArgLadder>::make(#name, #desc, #make_args)
            };
            quote
        });
        // let match_branches = self.match_branches(ty, &c_ty);
        let match_branches = self.variants.iter().map(|v| {
            let name = v.name();
            let ident = &v.ident;
            let variant = if let Some(first) = &v.fields.iter().next() {
                let ty = &first.ty;
                quote_spanned! { first.span() =>
                    #ident(
                        <#ty as ::discorsd::commands::CommandData<#c_ty>>::from_options(lower)?
                    )
                }
            } else {
                quote_spanned! { ident.span() =>
                    #ident
                }
            };
            quote_spanned! { v.ident.span() =>
                #name => ::std::result::Result::Ok(Self::#variant)
            }
        });
        let variants_array = self.variants_array();

        quote! {
            #command_data_impl for #generic_ty {
                // god that's ugly v2
                type Options =
                <
                    <
                        #first_variant_ty as ::discorsd::model::commands::CommandData<#c_ty>
                    >::Options as ::discorsd::model::commands::OptionsLadder
                >::Raise;

                fn from_options(
                    Self::Options { name, lower }: Self::Options,
                ) -> ::std::result::Result<Self, ::discorsd::errors::CommandParseError> {
                    match name.as_str() {
                        #(#match_branches,)*
                        _ => ::std::result::Result::Err(::discorsd::errors::CommandParseError::UnknownOption(
                            ::discorsd::errors::UnknownOption { name, options: &#variants_array }
                        ))
                    }
                }

                // god that's ugly
                type VecArg =
                <
                    <
                        #first_variant_ty as ::discorsd::model::commands::CommandData<#c_ty>
                    >::VecArg as ::discorsd::model::commands::VecArgLadder
                >::Raise;

                fn make_args(command: &#c_ty) -> ::std::vec::Vec<Self::VecArg> {
                    vec![#(#args),*]
                }

                type Choice = Self;
            }
        }
    }

    /// Enums where each variant is either a struct or a tuple with 2+ fields (that tuple thing
    /// might be a lie as to how well it works for tuples...)
    /// ```
    /// # const IGNORE: &str = stringify!(
    /// #[derive(CommandData)]
    /// # );
    /// enum InlineStructs {
    ///     ColorCommand { hex: String },
    ///     PersonCommand(
    /// # #[doc = r#"
    ///         #[command(rename = "name")]
    /// # "#]
    ///         String,
    /// # #[doc = r#"
    ///         #[command(rename = "age")]
    /// # "#]
    ///         u32
    ///     )
    /// }
    /// ```
    fn inline_structs(&self, ty: &Ident) -> TokenStream2 {
        let (command_data_impl_statement, c_ty) = command_data_impl(self.command_type.as_ref(), &self.generics);
        let from_option_branches = self.from_options_branches(ty, &c_ty);
        let variants_array = self.variants_array();
        let make_args_vec = self.make_args_vec(&c_ty);

        quote! {
            #command_data_impl_statement for #ty {
                // All inline struct enums are SubCommands
                type Options = ::discorsd::model::interaction::CommandOption;

                fn from_options(
                    Self::Options { name, lower: options }: Self::Options
                ) -> ::std::result::Result<Self, ::discorsd::errors::CommandParseError> {
                    use ::discorsd::errors::*;
                    match name.as_str() {
                        #from_option_branches
                        _ => ::std::result::Result::Err(CommandParseError::UnknownOption(UnknownOption {
                            name,
                            options: &#variants_array,
                        }))
                    }
                }

                // All inline struct enums are SubCommands
                type VecArg = ::discorsd::model::interaction::SubCommand;

                fn make_args(command: &#c_ty) -> ::std::vec::Vec<Self::VecArg> {
                    #make_args_vec
                }

                type Choice = Self;
            }
        }
    }
}

impl FromIterator<Variant> for Enum {
    fn from_iter<T: IntoIterator<Item=Variant>>(iter: T) -> Self {
        Self { variants: iter.into_iter().collect(), command_type: None, generics: Vec::new() }
    }
}