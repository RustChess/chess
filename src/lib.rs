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
#[macro_use(DeserializeFromStr, SerializeDisplay)]
extern crate serde_with;

pub mod bitboard;
pub mod const_map;
pub use const_map::Map;
pub mod formats;
pub mod game;
pub use game::{Cursor, Game, Id};
#[cfg(feature = "lichess")]
pub mod lichess;
pub mod moves;
#[cfg(test)]
mod perft;
pub mod position;
pub use position::{Board, File, Move, Piece, Player, Position, Rank, Role, Side, Square};
pub mod variant;
