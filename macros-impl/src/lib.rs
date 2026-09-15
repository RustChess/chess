use proc_macro::TokenStream;

#[cfg(feature = "serde")]
mod serde_str;

#[proc_macro_derive(Arbitrary, attributes(arbitrary))]
pub fn arbitrary(_: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Args, attributes(arg, clap, command, group))]
pub fn args(_: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Deserialize, attributes(serde))]
pub fn deserialize(_: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(DeserializeFromStr)]
pub fn deserialize_from_str(_input: TokenStream) -> TokenStream {
    #[cfg(feature = "serde")]
    return serde_str::deserialize(_input);

    #[cfg(not(feature = "serde"))]
    TokenStream::new()
}

#[proc_macro_derive(Serialize, attributes(serde))]
pub fn serialize(_: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(SerializeDisplay)]
pub fn serialize_display(_input: TokenStream) -> TokenStream {
    #[cfg(feature = "serde")]
    return serde_str::serialize(_input);

    #[cfg(not(feature = "serde"))]
    TokenStream::new()
}
