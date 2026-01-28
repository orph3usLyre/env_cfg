use heck::ToSnekCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Field, Fields, Lit, Meta, parse_macro_input, spanned::Spanned};

const SUPPORTED_STRUCT_ATTRIBUTES: &[&str] = &[r#"prefix = "<PREFIX>""#, "no_prefix"];
const SUPPORTED_FIELD_ATTRIBUTES: &[&str] = &[
    "skip",
    "nested",
    r#"env = "<VAR_NAME>""#,
    "default = <DEFAULT_VALUE>",
    r#"parse_with = "<PARSER_FN>""#,
];

#[derive(Debug, Clone)]
enum PrefixConfig {
    /// Use struct name as prefix (default behavior)
    StructName(String),
    /// Use custom prefix
    Custom(String),
    /// No prefix
    None,
}

impl PrefixConfig {
    fn as_prefix(&self) -> Option<&str> {
        match self {
            PrefixConfig::StructName(s) | PrefixConfig::Custom(s) => Some(s),
            PrefixConfig::None => None,
        }
    }

    fn apply_to_field(&self, field_name: &str) -> String {
        match self.as_prefix() {
            Some(prefix) => format!("{}_{}", prefix, field_name).to_ascii_uppercase(),
            None => field_name.to_ascii_uppercase(),
        }
    }
}

/// Derive macro for EnvConfig trait
///
/// By default, maps struct field names to STRUCT_NAME_FIELD_NAME in UPPER_SNAKE_CASE environment variables.
///
/// Supports struct-level attributes:
/// - `#[env_cfg(no_prefix)]` - disable prefix, use field names directly
/// - `#[env_cfg(prefix = "PREFIX")]` - use custom prefix instead of struct name
///
/// Supports field-level attributes:
/// - `#[env_cfg(skip)]` - skip this field (won't load from env) (must implement Default)
/// - `#[env_cfg(env = "VAR_NAME")]` - specify custom env var name
/// - `#[env_cfg(default = "value")]` - specify default value  
/// - `#[env_cfg(parse_with = "function_name")]` - use custom parser function (signature: `fn(String) -> T`)
/// - `#[env_cfg(nested)]` - treat field as nested EnvConfig struct (calls T::from_env())
///
/// # Feature: `trace`
///
/// When the `trace` feature is enabled, the generated `from_env()` method will emit a
/// `tracing::trace!` event with the loaded configuration struct. This requires the struct
/// to implement `Debug`.
///
#[proc_macro_derive(EnvConfig, attributes(env_cfg))]
pub fn derive_env_cfg(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Parse struct-level attributes for prefix configuration
    let prefix_config = match parse_struct_prefix_config(&input).map_err(|e| e.into_compile_error())
    {
        Ok(config) => config,
        Err(e) => return e.into(),
    };

    expand_env_cfg(input, &prefix_config)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_env_cfg(
    input: DeriveInput,
    prefix_config: &PrefixConfig,
) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;
    let name_str = name.to_string();
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            o => {
                return Err(syn::Error::new(
                    o.span(),
                    "EnvConfig can only be derived for structs with named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new(
                input.span(),
                "EnvConfig can only be derived for structs",
            ));
        }
    };

    let field_assignments: Result<Vec<_>, _> = fields
        .iter()
        .map(|field| generate_field_assignment(field, prefix_config))
        .collect();
    let field_assignments = field_assignments?;

    let expanded = quote! {
        impl ::env_cfg::EnvConfig for #name {
            type Error = ::env_cfg::EnvConfigError;

            fn from_env() -> Result<Self, Self::Error> {
                let __result = Self {
                    #(#field_assignments,)*
                };
                ::env_cfg::trace_config(&__result, #name_str);
                Ok(__result)
            }
        }
    };
    Ok(expanded)
}

fn parse_struct_prefix_config(input: &DeriveInput) -> syn::Result<PrefixConfig> {
    let struct_name = input.ident.to_string();

    // Convert PascalCase struct name to snake_case for the prefix
    let snake_case_struct_name = struct_name.to_snek_case();

    // Default behavior: use struct name as prefix
    let mut prefix_config = PrefixConfig::StructName(snake_case_struct_name);
    let mut existing_struct_attribute = false;

    // Check for struct-level attributes
    for attr in &input.attrs {
        if attr.path().is_ident("env_cfg") {
            if let Meta::List(meta_list) = &attr.meta {
                let nested_metas = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )?;

                for nested in nested_metas {
                    match nested {
                        Meta::Path(path) if path.is_ident("no_prefix") => {
                            if !existing_struct_attribute {
                                prefix_config = PrefixConfig::None;
                                existing_struct_attribute = true;
                            } else {
                                return Err(syn::Error::new(
                                    path.span(),
                                    "Cannot use no_prefix with other attributes",
                                ));
                            }
                        }
                        Meta::NameValue(name_value) if name_value.path.is_ident("prefix") => {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: Lit::Str(lit_str),
                                ..
                            }) = &name_value.value
                            {
                                if !existing_struct_attribute {
                                    prefix_config = PrefixConfig::Custom(lit_str.value());
                                    existing_struct_attribute = true;
                                } else {
                                    return Err(syn::Error::new(
                                        name_value.span(),
                                        "Cannot use prefix with other attributes",
                                    ));
                                }
                            }
                        }
                        o => {
                            return Err(syn::Error::new(
                                o.span(),
                                format!(
                                    "Unsupported struct attribute. Supported attributes include: {SUPPORTED_STRUCT_ATTRIBUTES:?}"
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(prefix_config)
}

fn is_option_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(type_path) = ty {
        if type_path.qself.is_none() {
            if let Some(segment) = type_path.path.segments.last() {
                return segment.ident == "Option";
            }
        }
    }
    false
}

fn generate_field_assignment(
    field: &Field,
    prefix_config: &PrefixConfig,
) -> syn::Result<proc_macro2::TokenStream> {
    let field_name = field.ident.as_ref().unwrap();
    let field_name_str = field_name.to_string();
    let field_type = &field.ty;

    // Parse attributes
    let mut env_name = prefix_config.apply_to_field(&field_name_str);
    let mut default_expr: Option<syn::Expr> = None;
    let mut skip = false;
    let mut parse_with: Option<syn::Expr> = None;
    let mut is_nested = false;

    for attr in &field.attrs {
        if attr.path().is_ident("env_cfg") {
            if let Meta::List(meta_list) = &attr.meta {
                let nested_metas = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )?;

                for nested in nested_metas {
                    match nested {
                        Meta::Path(path) if path.is_ident("skip") => {
                            skip = true;
                        }
                        Meta::Path(path) if path.is_ident("nested") => {
                            is_nested = true;
                        }
                        Meta::NameValue(name_value) if name_value.path.is_ident("env") => {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: Lit::Str(lit_str),
                                ..
                            }) = &name_value.value
                            {
                                env_name = lit_str.value();
                            }
                        }
                        Meta::NameValue(name_value) if name_value.path.is_ident("default") => {
                            default_expr = Some(name_value.value.clone());
                        }
                        Meta::NameValue(name_value) if name_value.path.is_ident("parse_with") => {
                            parse_with = Some(name_value.value.clone());
                        }
                        other => {
                            return Err(syn::Error::new(
                                other.span(),
                                format!(
                                    "Unsupported field attribute. Supported attributes: {SUPPORTED_FIELD_ATTRIBUTES:?}"
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }

    // Validate attribute combinations
    if skip && (default_expr.is_some() || parse_with.is_some() || is_nested) {
        return Err(syn::Error::new(
            field.span(),
            "Cannot use 'skip' with other attributes",
        ));
    }

    if is_nested && (default_expr.is_some() || parse_with.is_some()) {
        return Err(syn::Error::new(
            field.span(),
            "Cannot use 'nested' with 'default' or 'parse_with' attributes",
        ));
    }

    if parse_with.is_some() && default_expr.is_some() {
        return Err(syn::Error::new(
            field.span(),
            "Cannot use both 'parse_with' and 'default' attributes on the same field",
        ));
    }

    // Handle skipped fields
    if skip {
        return Ok(quote! {
            #field_name: Default::default()
        });
    }

    // Handle nested EnvConfig structs
    if is_nested {
        return Ok(quote! {
            #field_name: #field_type::from_env()
                .map_err(|e| ::env_cfg::EnvConfigError::Nested(
                    stringify!(#field_type).to_string(),
                    Box::new(e)
                ))?
        });
    }

    // Handle fields with custom parser
    if let Some(parser_fn) = parse_with {
        let parser_ident = if let syn::Expr::Lit(syn::ExprLit {
            lit: Lit::Str(lit_str),
            ..
        }) = &parser_fn
        {
            let fn_name = lit_str.value();
            syn::Ident::new(&fn_name, lit_str.span())
        } else {
            return Err(syn::Error::new(
                parser_fn.span(),
                "parse_with must be a string literal containing the function name",
            ));
        };

        return if is_option_type(field_type) {
            Ok(quote! {
                #field_name: ::env_cfg::env_var_optional_with_parser(#env_name, #parser_ident)?
            })
        } else {
            Ok(quote! {
                #field_name: ::env_cfg::env_var_with_parser(#env_name, #parser_ident)?
            })
        };
    }

    // Handle default
    if let Some(default) = default_expr {
        return Ok(quote! {
            #field_name: ::env_cfg::env_var_or_parse(#env_name, #default)?
        });
    }

    // Standard field - type determines behavior (T vs Option<T>)
    if is_option_type(field_type) {
        Ok(quote! {
            #field_name: ::env_cfg::env_var_optional(#env_name)?
        })
    } else {
        Ok(quote! {
            #field_name: ::env_cfg::env_var(#env_name)?
        })
    }
}
