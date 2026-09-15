// Adapted from serde_with_macros: https://github.com/jonasbb/serde_with
// Licensed under Apache-2.0 or MIT.

use proc_macro::TokenStream;

use quote::{ToTokens, quote};
use syn::{DeriveInput, Generics, parse_quote, punctuated::Punctuated, token::Comma};

pub fn deserialize(input: TokenStream) -> TokenStream {
    let input = match syn::parse(input) {
        Ok(input) => input,
        Err(error) => return error.into_compile_error().into(),
    };
    deserialize_from_str(input).into()
}

fn deserialize_from_str(mut input: DeriveInput) -> proc_macro2::TokenStream {
    let ident = input.ident;
    let where_clause = &mut input.generics.make_where_clause().predicates;
    where_clause.push(parse_quote!(Self: ::core::str::FromStr));
    where_clause.push(parse_quote!(
        <Self as ::core::str::FromStr>::Err: ::core::fmt::Display
    ));
    let de_impl_generics = DeImplGenerics(&input.generics);
    let (_, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #de_impl_generics ::serde::Deserialize<'de> for #ident #ty_generics #where_clause {
            fn deserialize<__D>(deserializer: __D) -> ::core::result::Result<Self, __D::Error>
            where
                __D: ::serde::Deserializer<'de>,
            {
                struct Helper<__S>(::core::marker::PhantomData<__S>);

                impl<'de, __S> ::serde::de::Visitor<'de> for Helper<__S>
                where
                    __S: ::core::str::FromStr,
                    <__S as ::core::str::FromStr>::Err: ::core::fmt::Display,
                {
                    type Value = __S;

                    fn expecting(
                        &self,
                        formatter: &mut ::core::fmt::Formatter<'_>,
                    ) -> ::core::fmt::Result {
                        formatter.write_str("a string")
                    }

                    fn visit_str<__E>(
                        self,
                        value: &str,
                    ) -> ::core::result::Result<Self::Value, __E>
                    where
                        __E: ::serde::de::Error,
                    {
                        value.parse().map_err(::serde::de::Error::custom)
                    }

                    fn visit_bytes<__E>(
                        self,
                        value: &[u8],
                    ) -> ::core::result::Result<Self::Value, __E>
                    where
                        __E: ::serde::de::Error,
                    {
                        let value = ::core::str::from_utf8(value)
                            .map_err(::serde::de::Error::custom)?;
                        self.visit_str(value)
                    }
                }

                deserializer.deserialize_str(Helper(::core::marker::PhantomData))
            }
        }
    }
}

pub fn serialize(input: TokenStream) -> TokenStream {
    let input = match syn::parse(input) {
        Ok(input) => input,
        Err(error) => return error.into_compile_error().into(),
    };
    serialize_display(input).into()
}

fn serialize_display(mut input: DeriveInput) -> proc_macro2::TokenStream {
    let ident = input.ident;
    input.generics.make_where_clause().predicates.push(parse_quote!(Self: ::core::fmt::Display));
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_generics ::serde::Serialize for #ident #ty_generics #where_clause {
            fn serialize<__S>(
                &self,
                serializer: __S,
            ) -> ::core::result::Result<__S::Ok, __S::Error>
            where
                __S: ::serde::Serializer,
            {
                serializer.collect_str(self)
            }
        }
    }
}

struct DeImplGenerics<'a>(&'a Generics);

impl ToTokens for DeImplGenerics<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let mut generics = self.0.clone();
        generics.params = Some(parse_quote!('de))
            .into_iter()
            .chain(generics.params)
            .collect::<Punctuated<_, Comma>>();
        let (impl_generics, _, _) = generics.split_for_impl();
        impl_generics.to_tokens(tokens);
    }
}
