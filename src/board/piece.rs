use crate::{
    Side, Square,
    square::{File, Rank},
};

use super::Bitboard;

use Player::*;
use Rank::*;
use Role::*;

/// A chess piece, for instance a white pawn or a black queen.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Piece {
    pub player: Player,
    pub role: Role,
}

crate::finite_set!(
    /// Black and white
    Player,
    Players,
    PlayerTable,
    {
        Black = 0 as black,
        White = 1 as white,
    }
);

crate::finite_set!(
    /// A chess piece role, such as pawn, knight, bishop, etc.
    Role,
    Roles,
    RoleTable,
    {
        Pawn = 1 as pawn,
        Knight = 2 as knight,
        Bishop = 3 as bishop,
        Rook = 5 as rook,
        Queen = 9 as queen,
        King = 4 as king,
    }
);

impl Piece {
    #[inline]
    pub const fn from_char(c: char) -> Option<Self> {
        let player = if c.is_ascii_lowercase() { Black } else { White };
        match Role::from_char(c) {
            Some(role) => Some(Self { player, role }),
            None => None,
        }
    }

    #[inline]
    #[track_caller]
    pub(crate) const fn panicky_from_char(c: char) -> Self {
        match Self::from_char(c) {
            Some(piece) => piece,
            None => panic!("invalid piece character"),
        }
    }

    #[inline]
    pub const fn char(self) -> char {
        match self.player {
            Black => self.role.black(),
            White => self.role.white(),
        }
    }

    // Eq::eq is not const
    #[inline]
    pub const fn eq(self, other: Piece) -> bool {
        self.player.eq(other.player) && self.role.eq(other.role)
    }
}

impl Player {
    #[inline]
    pub const fn is_black(self) -> bool {
        matches!(self, Black)
    }

    #[inline]
    pub const fn is_white(self) -> bool {
        matches!(self, White)
    }

    #[inline]
    pub const fn other(self) -> Player {
        match self {
            Black => White,
            White => Black,
        }
    }

    #[inline]
    pub const fn backrank(self) -> Rank {
        match self {
            Black => Eight,
            White => One,
        }
    }

    #[inline]
    pub const fn pawn_start_rank(self) -> Rank {
        match self {
            Black => Seven,
            White => Two,
        }
    }

    #[inline]
    pub const fn promotion_rank(self) -> Rank {
        match self {
            Black => One,
            White => Eight,
        }
    }

    /// `White.pawn()`. `Role::of` is the inverse spelling for `Pawn.of(White)`.
    #[inline]
    pub const fn pawn(self) -> Piece {
        Pawn.of(self)
    }

    #[inline]
    pub const fn knight(self) -> Piece {
        Knight.of(self)
    }

    #[inline]
    pub const fn bishop(self) -> Piece {
        Bishop.of(self)
    }

    #[inline]
    pub const fn rook(self) -> Piece {
        Rook.of(self)
    }

    #[inline]
    pub const fn queen(self) -> Piece {
        Queen.of(self)
    }

    #[inline]
    pub const fn king(self) -> Piece {
        King.of(self)
    }

    /// The square the king moves to when castling on this side.
    #[inline]
    pub const fn castle_king_to(self, side: Side) -> Square {
        Square::new(side.king_to_file(), self.backrank())
    }

    /// The square the rook moves from when castling from this file.
    #[inline]
    pub const fn castle_rook_from(self, file: File) -> Square {
        Square::new(file, self.backrank())
    }

    /// The square the rook moves to when castling on this side.
    #[inline]
    pub const fn castle_rook_to(self, side: Side) -> Square {
        Square::new(side.rook_to_file(), self.backrank())
    }

    /// Squares that must be empty when castling with this king and rook.
    #[inline]
    pub const fn castle_empty_path(self, king_from: Square, rook_file: File) -> Bitboard {
        let side = Side::of_rook(king_from, rook_file);
        let king_to = self.castle_king_to(side);
        let rook_from = self.castle_rook_from(rook_file);
        let rook_to = self.castle_rook_to(side);

        let king_path = king_from.between(king_to).with(king_to);
        let rook_path = rook_from.between(rook_to).with(rook_to);

        // interval between king and rook, excluding endpoints
        king_path
            .union(rook_path)
            .difference(Bitboard::from_square(king_from))
            .difference(Bitboard::from_square(rook_from))
    }

    /// Squares the king occupies or crosses when castling on this side.
    #[inline]
    pub const fn castle_king_path(self, king_from: Square, side: Side) -> Bitboard {
        let king_to = self.castle_king_to(side);
        king_from.between(king_to).with(king_from).with(king_to)
    }
}

impl<T> Players<T> {
    #[inline]
    pub fn swap(self) -> Players<T> {
        Players { black: self.white, white: self.black }
    }
}

impl Role {
    #[inline]
    pub const fn from_char(c: char) -> Option<Self> {
        Some(match c {
            'p' | 'P' => Pawn,
            'n' | 'N' => Knight,
            'b' | 'B' => Bishop,
            'r' | 'R' => Rook,
            'q' | 'Q' => Queen,
            'k' | 'K' => King,
            _ => return None,
        })
    }

    #[track_caller]
    #[inline]
    pub(crate) const fn panicky_from_char(c: char) -> Self {
        match Self::from_char(c) {
            Some(role) => role,
            None => panic!("invalid role"),
        }
    }

    /// `Bishop.of(White)`
    #[inline]
    pub const fn of(self, player: Player) -> Piece {
        Piece { player, role: self }
    }

    #[inline]
    pub const fn lower(self) -> char {
        match self {
            Pawn => 'p',
            Knight => 'n',
            Bishop => 'b',
            Rook => 'r',
            Queen => 'q',
            King => 'k',
        }
    }

    #[inline]
    pub const fn upper(self) -> char {
        match self {
            Pawn => 'P',
            Knight => 'N',
            Bishop => 'B',
            Rook => 'R',
            Queen => 'Q',
            King => 'K',
        }
    }

    #[inline]
    pub const fn figurine(self) -> char {
        match self {
            Pawn => '♙',
            Knight => '♘',
            Bishop => '♗',
            Rook => '♖',
            Queen => '♕',
            King => '♔',
        }
    }

    #[inline]
    pub const fn black(self) -> char {
        self.lower()
    }

    #[inline]
    pub const fn white(self) -> char {
        self.upper()
    }
}
