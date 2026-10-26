use proc_macro::TokenStream;

#[cfg(feature = "clap")]
mod clap_enum_arg;
mod clone_copy;
mod data;

#[cfg(feature = "serde")]
mod serde_str;

/// Derives the standard data traits.
///
/// `string` uses `DeserializeFromStr` and `SerializeDisplay` for Serde. Traits listed in
/// `custom(...)` must be implemented by the annotated type instead. `custom(Arbitrary)` supports
/// a handwritten generator. `custom(EqOrd)` expects handwritten `PartialEq` and `Ord`
/// implementations and supplies `Eq` and `PartialOrd`; the handwritten implementations must
/// agree that `a == b` if and only if `a.cmp(b) == Ordering::Equal`.
#[proc_macro_attribute]
pub fn data(attributes: TokenStream, item: TokenStream) -> TokenStream {
    data::expand(attributes, item)
}

#[proc_macro_derive(Arbitrary, attributes(arbitrary))]
pub fn arbitrary(_: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Args, attributes(arg, clap, command, group))]
pub fn args(_: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(CloneCopy)]
pub fn clone_copy(input: TokenStream) -> TokenStream {
    clone_copy::derive(input)
}

#[proc_macro_derive(EnumArg)]
pub fn enum_arg(input: TokenStream) -> TokenStream {
    #[cfg(feature = "clap")]
    return clap_enum_arg::derive(input);

    #[cfg(not(feature = "clap"))]
    {
        let _ = input;
        TokenStream::new()
    }
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

#[proc_macro_derive(Subcommand, attributes(arg, clap, command, group))]
pub fn subcommand(_: TokenStream) -> TokenStream {
    TokenStream::new()
}
