//! # Chess
//!
//! Attempt at building an ergonomic, idiomatic, and fast foundation for Chess programming in Rust.

#[cfg(feature = "serde")]
#[macro_use(Deserialize, Serialize)]
extern crate serde;

#[cfg(feature = "serde")]
#[macro_use(DeserializeFromStr, SerializeDisplay)]
extern crate serde_with;

pub mod board;
pub mod finite;
pub mod formats;
pub mod game;
pub mod id;
pub mod moves;
pub mod position;
pub mod square;
pub mod variant;

#[cfg(feature = "lichess")]
pub mod lichess;

#[cfg(test)]
mod perft;

#[doc(inline)]
pub use board::{Board, Piece, Player, Role, Scharnagl};
#[doc(inline)]
pub use game::{Game, Node};
#[doc(inline)]
pub use id::Id;
#[doc(inline)]
pub use position::{Move, Position, Side};
#[doc(inline)]
pub use square::Square;
