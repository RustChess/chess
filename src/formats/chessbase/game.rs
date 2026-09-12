//! Chessbase game file format (CBG)

use core::num::NonZeroU32;

use crate::{
    Player,
    board::{PlayerTable, Role, RoleTable},
    formats::prelude::Framed,
    position::SideTable,
    square::{File, Square},
};

pub mod convert;
pub mod parse;

pub use parse::{PERMUTE, game, game_with_info, header, info, mode, position, tokens};

/// ChessBase game filename suffix.
pub const SUFFIX: &str = "cbg";

#[derive(Clone, Debug)]
pub struct Game {
    pub position: Position,
    pub tokens: Vec<Token>,
}

#[derive(Clone, Debug)]
pub struct Position {
    pub pieces: Pieces,
    pub turn: Player,
    pub castles: Castles,
    pub en_passant: Option<File>,
    pub round: NonZeroU32,
}

/// The tokens that moves are encoded as.
#[derive(Clone, Copy, Debug)]
pub enum Token {
    Pop,
    Push,
    Move(Move),
    Skip,
}

#[derive(Clone, Copy, Debug)]
pub enum Move {
    Null,
    // (x, y) moves: -1 <= x,y <= 1
    King(i8, i8),
    // x move; x = 2 or x = -2
    Castle(i8),
    // queen index 0, 1, or 2, and (x, y) moves: 1 <= x,y < 8
    Queen(u8, u8, u8),
    // rook index 0, 1, or 2, and (x, y) moves: 1 <= x,y < 8
    Rook(u8, u8, u8),
    // bishop index 0, 1, or 2, and (x, y) moves: 1 <= x,y < 8
    Bishop(u8, u8, u8),
    // knight index 0, 1, or 2, and (x, y) moves: -2 <= x,y < 2
    Knight(u8, i8, i8),
    // any file
    Pawn(File, PawnMove),
    // (from, to, role) squares and valid promotion Role
    //
    // It seems that "a fourth piece never promotes, and its movements
    // are always captured with multiple bytes".
    //
    // Only if the move is a pawn move that promotes, is the role valid.
    //
    // Any move could be encoded (UCI-style) with this (from, to) method,
    // but typically it's only done for moving "fourth pieces" or pawns.
    FromTo(Square, Square, Role),
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum PawnMove {
    One,
    Two,
    // captures, left/right from the perspective of the player
    Left,
    Right,
}

pub type Castles = PlayerTable<SideTable<bool>>;

/// Ten slots cover two original pieces plus eight pawns promoted to the same role.
pub type Pieces = PlayerTable<RoleTable<[Option<Square>; 10]>>;

impl Pieces {
    pub fn standard() -> Self {
        use Player::*;
        use Role::*;
        use Square::*;

        let mut pieces = Self::empty();
        pieces[White][King][0] = Some(E1);
        pieces[White][Queen][0] = Some(D1);
        pieces[White][Rook][..2].copy_from_slice(&[Some(A1), Some(H1)]);
        pieces[White][Bishop][..2].copy_from_slice(&[Some(C1), Some(F1)]);
        pieces[White][Knight][..2].copy_from_slice(&[Some(B1), Some(G1)]);
        pieces[White][Pawn][..8].copy_from_slice(&[
            Some(A2),
            Some(B2),
            Some(C2),
            Some(D2),
            Some(E2),
            Some(F2),
            Some(G2),
            Some(H2),
        ]);
        pieces[Black][King][0] = Some(E8);
        pieces[Black][Queen][0] = Some(D8);
        pieces[Black][Rook][..2].copy_from_slice(&[Some(A8), Some(H8)]);
        pieces[Black][Bishop][..2].copy_from_slice(&[Some(C8), Some(F8)]);
        pieces[Black][Knight][..2].copy_from_slice(&[Some(B8), Some(G8)]);
        pieces[Black][Pawn][..8].copy_from_slice(&[
            Some(A7),
            Some(B7),
            Some(C7),
            Some(D7),
            Some(E7),
            Some(F7),
            Some(G7),
            Some(H7),
        ]);
        pieces
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub first_game: u16,
    pub len: u32,
}

impl Header {
    pub const LEN: usize = 26;
}

#[derive(Clone, Copy, Debug)]
pub struct Info {
    /// Length of the game body following this information.
    pub len: usize,
    pub mode: Mode,
}

impl Info {
    pub const LEN: usize = 4;
}

impl Framed for Info {
    const LEN: usize = Info::LEN;

    fn total(&self) -> usize {
        Self::LEN + self.len
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Mode(pub u8);

impl Mode {
    #[inline]
    pub const fn is_freestyle(self) -> bool {
        // The individual meanings of bits one and three are unknown.
        self.0 & ((1 << 1) | (1 << 3)) != 0
    }

    #[inline]
    pub const fn has_special_encoding(self) -> bool {
        self.0 & (1 << 2) != 0
    }

    #[inline]
    pub const fn has_setup(self) -> bool {
        self.0 & (1 << 6) != 0
    }

    #[inline]
    pub const fn is_undecodable(self) -> bool {
        self.0 & (1 << 7) != 0
    }
}
