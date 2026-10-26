use std::collections::BTreeSet;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    DeriveInput, Meta, Path, Token, parse::Parse, parse::ParseStream, parse_macro_input,
    parse_quote, punctuated::Punctuated,
};

#[derive(Default)]
struct Arguments {
    custom: BTreeSet<String>,
    string: bool,
}

impl Parse for Arguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut arguments = Self::default();
        for argument in Punctuated::<Meta, Token![,]>::parse_terminated(input)? {
            match argument {
                Meta::Path(path) if path.is_ident("string") => arguments.string = true,
                Meta::List(list) if list.path.is_ident("custom") => {
                    let traits =
                        list.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
                    for path in traits {
                        let Some(ident) = path.get_ident() else {
                            return Err(syn::Error::new_spanned(
                                path,
                                "custom derives must be names",
                            ));
                        };
                        arguments.custom.insert(ident.to_string());
                    }
                }
                argument => {
                    return Err(syn::Error::new_spanned(
                        argument,
                        "data accepts `string` and `custom(...)` arguments",
                    ));
                }
            }
        }
        Ok(arguments)
    }
}

pub fn expand(attributes: TokenStream, item: TokenStream) -> TokenStream {
    let arguments = parse_macro_input!(attributes as Arguments);
    let input = parse_macro_input!(item as DeriveInput);
    expand_input(arguments, input).unwrap_or_else(syn::Error::into_compile_error).into()
}

fn expand_input(arguments: Arguments, input: DeriveInput) -> syn::Result<TokenStream2> {
    if let Some(custom) = arguments
        .custom
        .iter()
        .find(|custom| matches!(custom.as_str(), "Eq" | "Ord" | "PartialEq" | "PartialOrd"))
    {
        return Err(syn::Error::new_spanned(
            &input.ident,
            format!("use `custom(EqOrd)` instead of `custom({custom})`"),
        ));
    }

    let mut derives = vec![
        derive("Arbitrary", quote!(::macros::Arbitrary)),
        derive("Clone", quote!(Clone)),
        derive("Debug", quote!(Debug)),
        derive("Eq", quote!(Eq)),
        derive("Ord", quote!(Ord)),
        derive("PartialEq", quote!(PartialEq)),
        derive("PartialOrd", quote!(PartialOrd)),
    ];
    if arguments.string {
        derives.push(derive("DeserializeFromStr", quote!(::macros::DeserializeFromStr)));
        derives.push(derive("SerializeDisplay", quote!(::macros::SerializeDisplay)));
    } else {
        derives.push(derive("Deserialize", quote!(::macros::Deserialize)));
        derives.push(derive("Serialize", quote!(::macros::Serialize)));
    }

    let mut available = derives.iter().map(|(name, _)| name.as_str()).collect::<BTreeSet<_>>();
    available.insert("EqOrd");
    if let Some(custom) =
        arguments.custom.iter().find(|custom| !available.contains(custom.as_str()))
    {
        return Err(syn::Error::new_spanned(
            &input.ident,
            format!("data does not derive `{custom}` with these arguments"),
        ));
    }
    let custom_eq_ord = arguments.custom.contains("EqOrd");
    let derives = derives.into_iter().filter_map(|(name, derive)| {
        let custom = arguments.custom.contains(&name)
            || (custom_eq_ord
                && matches!(name.as_str(), "Eq" | "Ord" | "PartialEq" | "PartialOrd"));
        (!custom).then_some(derive)
    });

    let name = &input.ident;
    let (_, type_generics, _) = input.generics.split_for_impl();
    let eq = custom_eq_ord.then(|| {
        let mut generics = input.generics.clone();
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#name #type_generics: ::core::cmp::PartialEq));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        quote! {
            impl #impl_generics ::core::cmp::Eq for #name #type_generics #where_clause {}
        }
    });
    let partial_ord = custom_eq_ord.then(|| {
        let mut generics = input.generics.clone();
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#name #type_generics: ::core::cmp::Ord));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        quote! {
            impl #impl_generics ::core::cmp::PartialOrd for #name #type_generics #where_clause {
                fn partial_cmp(&self, other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                    ::core::option::Option::Some(::core::cmp::Ord::cmp(self, other))
                }
            }
        }
    });

    Ok(quote! {
        #[derive(#(#derives),*)]
        #input

        #eq
        #partial_ord
    })
}

fn derive(name: &str, tokens: TokenStream2) -> (String, TokenStream2) {
    (name.to_owned(), tokens)
}
