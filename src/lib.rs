//! # Chess
//!
//! Attempt at building an ergonomic, idiomatic, and fast foundation for Chess programming in Rust.

#[cfg(feature = "serde")]
#[macro_use(Deserialize, Serialize)]
extern crate serde;

#[cfg(feature = "serde")]
#[macro_use(DeserializeFromStr, SerializeDisplay)]
extern crate serde_with;

pub mod bitboard;
pub mod finite;
pub mod formats;
pub mod game;
#[doc(inline)]
pub use game::{Game, Node};
pub mod id;
#[doc(inline)]
pub use id::Id;
#[cfg(feature = "lichess")]
pub mod lichess;
pub mod moves;
#[cfg(test)]
mod perft;
pub mod position;
#[doc(inline)]
pub use position::{Board, Move, Piece, Player, Position, Role, Side, Square};
pub mod variant;
