//! # Chess
//!
//! Attempt at building an ergonomic, idiomatic, and fast foundation for Chess programming in Rust.
//!
//! ```
//! use chess::Position;
//! ```

pub mod bitboard;
pub mod formats;
#[cfg(feature = "lichess")]
pub mod lichess;
pub mod position;
pub use position::{Move, Position};

pub use position::Map;
