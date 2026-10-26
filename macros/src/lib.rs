//! Feature-switchable derive macros.
//!
//! This crate exports empty implementations of its derive macros by default. Enabling the
//! corresponding `arbitrary`, `clap`, or `serde` feature replaces them with functional derives,
//! allowing consumers to write unconditional `derive` attributes instead of repeating
//! `cfg_attr` throughout their source.
//!
//! ```ignore
//! #[macro_use(Deserialize, Serialize)]
//! extern crate macros;
//!
//! #[derive(Deserialize, Serialize)]
//! struct Value;
//! ```
//!
//! Consumers remain responsible for direct runtime dependencies required by generated code. For
//! example, a consumer's `serde` feature should enable both `dep:serde` and `macros/serde`.

extern crate self as macros;

#[cfg(feature = "arbitrary")]
pub use derive_arbitrary::Arbitrary;
#[cfg(not(feature = "arbitrary"))]
pub use macros_impl::Arbitrary;

#[cfg(feature = "clap")]
pub use clap_derive::{Args, Subcommand};
#[cfg(not(feature = "clap"))]
pub use macros_impl::{Args, Subcommand};
pub use macros_impl::{CloneCopy, EnumArg, data};

#[cfg(not(feature = "serde"))]
pub use macros_impl::{Deserialize, Serialize};
#[cfg(feature = "serde")]
pub use serde_derive::{Deserialize, Serialize};

pub use macros_impl::{DeserializeFromStr, SerializeDisplay};

#[cfg(feature = "serde")]
pub trait DeserializeOwned: serde::de::DeserializeOwned {}
#[cfg(feature = "serde")]
impl<T: serde::de::DeserializeOwned> DeserializeOwned for T {}

#[cfg(feature = "serde")]
pub trait Serialize: serde::Serialize {}
#[cfg(feature = "serde")]
impl<T: serde::Serialize> Serialize for T {}

#[cfg(not(feature = "serde"))]
pub trait DeserializeOwned {}
#[cfg(not(feature = "serde"))]
impl<T> DeserializeOwned for T {}

#[cfg(not(feature = "serde"))]
pub trait Serialize {}
#[cfg(not(feature = "serde"))]
impl<T> Serialize for T {}

#[cfg(feature = "arbitrary")]
pub trait ArbitraryOwned: for<'a> arbitrary::Arbitrary<'a> {}
#[cfg(feature = "arbitrary")]
impl<T> ArbitraryOwned for T where T: for<'a> arbitrary::Arbitrary<'a> {}

#[cfg(not(feature = "arbitrary"))]
pub trait ArbitraryOwned {}
#[cfg(not(feature = "arbitrary"))]
impl<T> ArbitraryOwned for T {}

pub trait Data:
    ArbitraryOwned + Clone + core::fmt::Debug + Eq + Ord + DeserializeOwned + Serialize
{
}

impl<T> Data for T where
    T: ArbitraryOwned + Clone + core::fmt::Debug + Eq + Ord + DeserializeOwned + Serialize
{
}

/// Whether a value equals its default, for `serde(skip_serializing_if)`.
pub fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    value == &T::default()
}

#[cfg(test)]
mod test;
