macro_rules! handle_attribute {
    (
        $self:ident $ty:ty =>
        $(
            $group_name:literal: $nested_meta_pattern:pat, $path:ident =>
            $(
                $(#[doc = $doc:literal])+
                [$str:literal => $($str_effect:tt)*]
            )+
        ),+
    ) => {
        impl $ty {
            fn handle_attribute($self: &mut Self, attr: &syn::Attribute) {
                use syn::spanned::Spanned;
                use quote::ToTokens;
                const OPTIONS: &[(&[(&str, &str)], &str)] = &[
                    $(
                        (
                            &[
                                $(
                                    ($str, concat!($($doc, "\n\t  "),+))
                                ),+
                            ],
                            $group_name
                        )
                    ),+
                ];
                let options_finding_error = |n_span, ident: &str, default| {
                    OPTIONS.iter()
                        .filter_map(|(opts, patt)|
                            opts.iter()
                                .filter(|(opt, _)| *opt == ident)
                                .next()
                                .map(|(_, doc)| (patt, doc))
                        )
                        .fold(
                            Diagnostic::spanned(n_span, Level::Error, format!("{} `{}`", default, ident)),
                            |d, (patt, doc)| {
                                d.help(format!("but `{}{}` is an option", ident, patt))
                                    .note(doc.trim().to_string())
                            }
                        ).emit();
                };
                let meta = attr.parse_meta().expect("Failed to parse meta");
                match meta {
                    Meta::List(MetaList { nested, .. }) => {
                        for nested in nested {
                            let n_span = nested.span();
                            #[allow(unreachable_patterns)]
                            match nested {
                                $(
                                    NestedMeta::Meta($nested_meta_pattern) => {
                                        match $path.get_ident().map(Ident::to_string).as_deref() {
                                            $(
                                                Some($str) => {
                                                    // the closure is needed so that `str_effect` can use `?`
                                                    #[allow(
                                                        clippy::redundant_closure_call,
                                                        clippy::unnecessary_operation,
                                                    )]
                                                    if let Err(e) = (|| -> Result<(), syn::Error> {
                                                        $($str_effect)*; Ok(())
                                                    })() {
                                                        emit_error!(e)
                                                    }
                                                }
                                            )+
                                            Some(ident) => options_finding_error(
                                                n_span, ident, "Unrecognized option".to_string()
                                            ),
                                            None => emit_error!(
                                                n_span,
                                                format!("Unable to parse attribute `{}`", $path.to_token_stream())
                                            ),
                                        };
                                    }
                                )+
                                NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) => {
                                    match path.get_ident().map(Ident::to_string).as_deref() {
                                        Some(ident) => options_finding_error(
                                            n_span, ident,
                                            format!("Unrecognized option `{}` for", lit.to_token_stream()),
                                        ),
                                        None => emit_error!(
                                            n_span,
                                            format!("Unable to parse attribute `{}`", path.to_token_stream())
                                        ),
                                    };
                                }
                                NestedMeta::Meta(Meta::Path(path)) => {
                                    match path.get_ident().map(Ident::to_string).as_deref() {
                                        Some(ident) => options_finding_error(
                                            n_span, ident,
                                            "Unrecognized option".to_string(),
                                        ),
                                        None => emit_error!(
                                            n_span,
                                            format!("Unable to parse attribute `{}`", path.to_token_stream())
                                        ),
                                    };
                                }
                                NestedMeta::Meta(Meta::List(list)) => {
                                    match list.path.get_ident().map(Ident::to_string).as_deref() {
                                        Some(ident) => options_finding_error(
                                            n_span, ident,
                                            "Unrecognized option".to_string(),
                                        ),
                                        None => emit_error!(
                                            n_span,
                                            format!("Unable to parse attribute `{}`", list.path.to_token_stream())
                                        ),
                                    };
                                }
                                NestedMeta::Lit(lit) => emit_error!(
                                    n_span,
                                    format!("Unknown literal attribute `{}`", lit.to_token_stream())
                                ),
                            }
                        }
                    }
                    other => emit_error!(
                        &other,
                        format!("Unexpected meta `{}`", other.to_token_stream()),
                    ),
                }
            }
        }
    };
}