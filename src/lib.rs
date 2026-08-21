//! # Chess
//!
//! Attempt at building an ergonomic, idiomatic, and fast foundation for Chess programming in Rust.
//!
//! ```
//! use chess::Position;
//! ```

#[cfg(feature = "serde")]
#[macro_use(Serialize)]
extern crate serde;

#[cfg(feature = "serde")]
#[macro_use(SerializeDisplay)]
extern crate serde_with;

pub mod bitboard;
pub mod formats;
pub mod game;
#[cfg(feature = "lichess")]
pub mod lichess;
pub mod position;
pub use position::{Move, Position};

pub use position::Map;
