use crate::{Player, Square, board::Players, finite::Empty as _, square::File};

use File::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Castles(pub Players<Sides<Option<File>>>);

crate::finite_set!(
    /// A square that can hold an en-passant target.
    EnPassant,
    EnPassantTable {
        A3 = 0 as a3,
        B3 = 1 as b3,
        C3 = 2 as c3,
        D3 = 3 as d3,
        E3 = 4 as e3,
        F3 = 5 as f3,
        G3 = 6 as g3,
        H3 = 7 as h3,
        A6 = 8 as a6,
        B6 = 9 as b6,
        C6 = 10 as c6,
        D6 = 11 as d6,
        E6 = 12 as e6,
        F6 = 13 as f6,
        G6 = 14 as g6,
        H6 = 15 as h6,
    }
);

// This has a multitude of choices:
// a) queenside / kingside (based on queen/king starting sides in chess)
// b) long / short (based on travel of king in chess)
// c) a-side / h-side (based on left/right-most files in both chess and freestyle) - used in freestyle
// d) c-file / g-file (where the king lands in both chess and freestyle) - new invention
//
// The goal would be to have something that makes intuitive sense for Chess
// and remains correct for Freestyle.
//
// Note that "O-O-O" and "O-O" continue to be used in Freestyle.
// a) is wrong for Freestyle, c) is rare in Chess, d) is a new invention,
// even though c) is not quite right in terms of actual king travel in Freestyle,
// the move notation still reflects it.
//
// Also Side is a bit misleading, could also mean what we call Player
crate::finite_set!(
    /// A side of the board to castle toward.
    Side,
    Sides,
    SideTable,
    {
        King = 0 as king,
        Queen = 1 as queen,
    }
);

impl Castles {
    #[inline]
    pub const fn empty() -> Self {
        Self(Players::EMPTY)
    }

    #[inline]
    pub const fn chess() -> Self {
        use Side::*;
        let rooks = Sides { queen: Some(Queen.chess_rook()), king: Some(King.chess_rook()) };
        Self(Players { black: rooks, white: rooks })
    }

    #[inline]
    pub const fn chess_compatible(self) -> bool {
        self.is_subset(Self::chess())
    }

    #[inline]
    pub const fn is_subset(self, other: Self) -> bool {
        // This needs const eq, so can't use the Eq trait and define on Option<T>.
        const fn file_subset(left: Option<File>, right: Option<File>) -> bool {
            match (left, right) {
                (None, _) => true,
                (Some(left), Some(right)) => left.eq(right),
                (Some(_), None) => false,
            }
        }

        finite_for!(player in Player {
            finite_for!(side in Side {
                if !file_subset(self.get(player, side), other.get(player, side)) {
                    return false;
                }
            });
        });

        true
    }

    #[inline]
    pub const fn get(self, player: Player, side: Side) -> Option<File> {
        self.0.get(player).get(side)
    }

    #[inline]
    pub const fn has(self, player: Player, side: Side) -> bool {
        self.get(player, side).is_some()
    }

    #[inline]
    pub const fn set(&mut self, player: Player, side: Side, file: File) {
        *self.0.get_mut(player).get_mut(side) = Some(file);
    }

    #[inline]
    pub fn clear(&mut self, player: Player, side: Side) {
        self.0[player][side] = None;
    }

    #[inline]
    pub fn clear_player(&mut self, player: Player) {
        self.0[player] = Sides::EMPTY;
    }
}

impl EnPassant {
    #[inline]
    pub const fn from_square(square: Square) -> Option<Self> {
        use Square::*;
        Some(match square {
            A3 => EnPassant::A3,
            B3 => EnPassant::B3,
            C3 => EnPassant::C3,
            D3 => EnPassant::D3,
            E3 => EnPassant::E3,
            F3 => EnPassant::F3,
            G3 => EnPassant::G3,
            H3 => EnPassant::H3,
            A6 => EnPassant::A6,
            B6 => EnPassant::B6,
            C6 => EnPassant::C6,
            D6 => EnPassant::D6,
            E6 => EnPassant::E6,
            F6 => EnPassant::F6,
            G6 => EnPassant::G6,
            H6 => EnPassant::H6,
            _ => return None,
        })
    }

    #[inline]
    pub const fn square(self) -> Square {
        use EnPassant::*;
        match self {
            A3 => Square::A3,
            B3 => Square::B3,
            C3 => Square::C3,
            D3 => Square::D3,
            E3 => Square::E3,
            F3 => Square::F3,
            G3 => Square::G3,
            H3 => Square::H3,
            A6 => Square::A6,
            B6 => Square::B6,
            C6 => Square::C6,
            D6 => Square::D6,
            E6 => Square::E6,
            F6 => Square::F6,
            G6 => Square::G6,
            H6 => Square::H6,
        }
    }
}

impl Side {
    #[inline]
    pub const fn chess_rook(self) -> File {
        match self {
            Side::King => H,
            Side::Queen => A,
        }
    }

    #[inline]
    pub const fn of_rook(king: Square, rook: File) -> Self {
        use Side::*;
        if king.file().index() <= rook.index() { King } else { Queen }
    }

    /// The file the king moves to when castling on this side
    #[inline]
    pub const fn king_to_file(self) -> File {
        use Side::*;
        match self {
            King => G,
            Queen => C,
        }
    }

    /// The file the rook moves to when castling on this side.
    #[inline]
    pub const fn rook_to_file(self) -> File {
        use Side::*;
        match self {
            King => F,
            Queen => D,
        }
    }
}

impl TryFrom<Square> for EnPassant {
    type Error = ();

    fn try_from(square: Square) -> Result<Self, ()> {
        Self::from_square(square).ok_or(())
    }
}

impl From<EnPassant> for Square {
    fn from(en_passant: EnPassant) -> Self {
        en_passant.square()
    }
}
