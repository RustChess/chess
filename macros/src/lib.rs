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

#[cfg(feature = "arbitrary")]
pub use derive_arbitrary::Arbitrary;
#[cfg(not(feature = "arbitrary"))]
pub use macros_impl::Arbitrary;

#[cfg(feature = "clap")]
pub use clap_derive::Args;
#[cfg(not(feature = "clap"))]
pub use macros_impl::Args;

#[cfg(not(feature = "serde"))]
pub use macros_impl::{Deserialize, Serialize};
#[cfg(feature = "serde")]
pub use serde_derive::{Deserialize, Serialize};

pub use macros_impl::{DeserializeFromStr, SerializeDisplay};
