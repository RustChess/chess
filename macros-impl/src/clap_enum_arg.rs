use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Expr, Fields, Lit, Meta};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = match syn::parse(input) {
        Ok(input) => input,
        Err(error) => return error.into_compile_error().into(),
    };
    derive_enum_arg(input).unwrap_or_else(syn::Error::into_compile_error).into()
}

fn derive_enum_arg(input: DeriveInput) -> syn::Result<TokenStream2> {
    let enum_help = help(&input.attrs).map(|help| quote!(.help(#help)));
    let enum_name = input.ident;
    let variants = match input.data {
        Data::Enum(data) => data.variants,
        _ => {
            return Err(syn::Error::new(enum_name.span(), "EnumArg can only be derived for enums"));
        }
    };
    if variants.is_empty() {
        return Err(syn::Error::new(enum_name.span(), "EnumArg requires at least one variant"));
    }

    let mut variant_names = Vec::with_capacity(variants.len());
    let mut fields = Vec::with_capacity(variants.len());
    let mut flags = Vec::with_capacity(variants.len());
    let mut help_texts = Vec::with_capacity(variants.len());
    let mut default = None;
    for variant in variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new(
                variant.ident.span(),
                "EnumArg variants must have no fields",
            ));
        }
        if variant.attrs.iter().any(|attribute| attribute.path().is_ident("default")) {
            if default.is_some() {
                return Err(syn::Error::new(
                    variant.ident.span(),
                    "EnumArg can only have one default variant",
                ));
            }
            default = Some(variant.ident.clone());
        }
        flags.push(to_kebab_case(&variant.ident.to_string()));
        fields.push(to_snake_case(&variant.ident.to_string()));
        help_texts.push(help(&variant.attrs).map(|help| quote!(.help(#help))));
        variant_names.push(variant.ident);
    }

    let enum_flag = to_kebab_case(&enum_name.to_string());
    let enum_field = to_snake_case(&enum_name.to_string());
    let group = format!("{enum_name}Arg");
    let arguments = fields.iter().zip(&flags).zip(&help_texts).map(|((field, flag), help)| {
        quote! {
            let command = command.arg(
                ::clap::Arg::new(#field)
                    .long(#flag)
                    .action(::clap::ArgAction::SetTrue)
                    .group(#group)
                    #help,
            );
        }
    });
    let selected = fields.iter().zip(&variant_names).map(|(field, variant)| {
        quote! {
            if matches.get_flag(#field) {
                return ::core::result::Result::Ok(Self::#variant);
            }
        }
    });
    let updated = fields.iter().zip(&variant_names).map(|(field, variant)| {
        quote! {
            if matches.value_source(#field)
                == ::core::option::Option::Some(::clap::parser::ValueSource::CommandLine)
            {
                *self = Self::#variant;
                return ::core::result::Result::Ok(());
            }
        }
    });
    let possible_values = variant_names.iter().map(|variant| quote!(Self::#variant));
    let possible_value = variant_names.iter().zip(&fields).map(|(variant, field)| {
        quote! {
            Self::#variant => ::core::option::Option::Some(
                ::clap::builder::PossibleValue::new(#field),
            ),
        }
    });
    let fallback = match default {
        Some(variant) => quote!(::core::result::Result::Ok(Self::#variant)),
        None => quote! {
            ::core::result::Result::Err(::clap::Error::raw(
                ::clap::error::ErrorKind::MissingRequiredArgument,
                ::core::concat!("missing ", #enum_flag),
            ))
        },
    };

    Ok(quote! {
        #[automatically_derived]
        impl ::clap::ValueEnum for #enum_name {
            fn value_variants<'a>() -> &'a [Self] {
                &[#(#possible_values),*]
            }

            fn to_possible_value(&self) -> ::core::option::Option<::clap::builder::PossibleValue> {
                match self {
                    #(#possible_value)*
                }
            }
        }

        #[automatically_derived]
        impl ::clap::Args for #enum_name {
            fn group_id() -> ::core::option::Option<::clap::Id> {
                ::core::option::Option::Some(::clap::Id::from(#group))
            }

            fn augment_args(command: ::clap::Command) -> ::clap::Command {
                let command = command.group(::clap::ArgGroup::new(#group).multiple(false));
                let command = command.arg(
                    ::clap::Arg::new(#enum_field)
                        .long(#enum_flag)
                        .action(::clap::ArgAction::Set)
                        .value_parser(::clap::builder::EnumValueParser::<Self>::new())
                        .group(#group)
                        #enum_help,
                );
                #(#arguments)*
                command
            }

            fn augment_args_for_update(command: ::clap::Command) -> ::clap::Command {
                Self::augment_args(command)
            }
        }

        #[automatically_derived]
        impl ::clap::FromArgMatches for #enum_name {
            fn from_arg_matches(
                matches: &::clap::ArgMatches,
            ) -> ::core::result::Result<Self, ::clap::Error> {
                if let ::core::option::Option::Some(value) = matches.get_one::<Self>(#enum_field) {
                    return ::core::result::Result::Ok(*value);
                }
                #(#selected)*
                #fallback
            }

            fn update_from_arg_matches(
                &mut self,
                matches: &::clap::ArgMatches,
            ) -> ::core::result::Result<(), ::clap::Error> {
                if matches.value_source(#enum_field)
                    == ::core::option::Option::Some(::clap::parser::ValueSource::CommandLine)
                {
                    *self = *matches
                        .get_one::<Self>(#enum_field)
                        .expect("EnumArg value is present");
                    return ::core::result::Result::Ok(());
                }
                #(#updated)*
                ::core::result::Result::Ok(())
            }
        }
    })
}

fn help(attributes: &[Attribute]) -> Option<String> {
    let lines = attributes.iter().filter_map(|attribute| {
        if !attribute.path().is_ident("doc") {
            return None;
        }
        let Meta::NameValue(meta) = &attribute.meta else { return None };
        let Expr::Lit(expression) = &meta.value else { return None };
        let Lit::Str(text) = &expression.lit else { return None };
        Some(text.value().trim().to_owned())
    });
    let help = lines.collect::<Vec<_>>().join("\n");
    let help = help.strip_suffix('.').unwrap_or(&help).to_owned();
    (!help.is_empty()).then_some(help)
}

fn to_kebab_case(name: &str) -> String {
    convert_case(name, '-')
}

fn to_snake_case(name: &str) -> String {
    convert_case(name, '_')
}

fn convert_case(name: &str, separator: char) -> String {
    let mut converted = String::new();
    for (index, character) in name.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            converted.push(separator);
        }
        converted.extend(character.to_lowercase());
    }
    converted
}
