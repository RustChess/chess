//! # Chess
//!
//! Attempt at building an ergonomic, idiomatic, and fast foundation for Chess programming in Rust.

mod arbitrary;

#[macro_use(Deserialize, DeserializeFromStr, Serialize, SerializeDisplay)]
extern crate macros;

pub mod board;
#[macro_use]
pub mod finite;
pub mod formats;
pub mod game;
pub mod id;
pub mod position;
pub mod puzzle;
pub mod square;

#[cfg(test)]
mod perft;

#[doc(inline)]
pub use board::{Board, Piece, Player, Role, Scharnagl};
#[doc(inline)]
pub use game::{Game, PositionId};
#[doc(inline)]
pub use id::Id;
#[doc(inline)]
pub use position::{Move, Position, Side};
#[doc(inline)]
pub use square::Square;
