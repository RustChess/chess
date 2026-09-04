use core::ops;

use crate::board::Bitboard;

use super::{Direction, Square};

/// Square Geometry API.
impl Square {
    pub const fn checked_add(self, direction: Direction) -> Option<Square> {
        let square = self as i8;
        let target = square + direction as i8;
        // Equivalent to splitting square + step into file + rank,
        // adding coordinates, and then checking if file + rank are in 0..=7
        let file_diff = (target & 0x7) - (square & 0x7);
        if target >= 0 && target < 64 && file_diff >= -2 && file_diff <= 2 {
            Some(Square::panicky_from_index(target as u8))
        } else {
            None
        }
    }

    // the full line through the two squares
    pub const fn full_ray(self, other: Square) -> Bitboard {
        Bitboard::FULL_RAYS[self as usize][other as usize]
    }

    // The row-major half-open interval [min(self, other), max(self, other)).
    //
    // For d2, g5, after also removing the first square:
    //
    // 8  . . . . . . . .
    // 7  . . . . . . . .
    // 6  . . . . . . . .
    // 5  x x x x x x . .
    // 4  x x x x x x x x
    // 3  x x x x x x x x
    // 2  . . . . x x x x
    // 1  . . . . . . . .
    //
    //    a b c d e f g h
    const fn index_range(self, other: Square) -> Bitboard {
        Bitboard((!0 << self as u32) ^ (!0 << other as u32))
    }

    // The row-major squares after this one, excluding self..
    // For d2, this includes e2..h8 and excludes a1..d2.
    //
    // For d2:
    //
    // 8  x x x x x x x x
    // 7  x x x x x x x x
    // 6  x x x x x x x x
    // 5  x x x x x x x x
    // 4  x x x x x x x x
    // 3  x x x x x x x x
    // 2  . . . . x x x x
    // 1  . . . . . . . .
    //
    //    a b c d e f g h
    pub const fn index_after(self) -> Bitboard {
        Bitboard(!0 << (self as u32 + 1))
    }

    // The row-major squares before this one, excluding self.
    // For d2, this includes a1..c2 and excludes d2..h8.
    pub const fn index_before(self) -> Bitboard {
        Bitboard((1 << self as u32) - 1)
    }

    pub const fn east(self) -> Bitboard {
        self.index_after().intersection(Bitboard::from_rank(self.rank()))
    }

    pub const fn west(self) -> Bitboard {
        self.index_before().intersection(Bitboard::from_rank(self.rank()))
    }

    pub const fn between(self, other: Square) -> Bitboard {
        // Intersecting the index range with the geometric ray leaves only the
        // ray segment between the endpoints.
        self.full_ray(other).intersection(self.index_range(other)).without_first()
    }

    pub const fn aligned(self, b: Square, c: Square) -> bool {
        self.full_ray(b).contains(c)
    }
}

impl ops::Add<Direction> for Square {
    type Output = Option<Square>;

    fn add(self, direction: Direction) -> Option<Square> {
        self.checked_add(direction)
    }
}

impl ops::Add<&[Direction]> for Square {
    type Output = Bitboard;

    fn add(self, directions: &[Direction]) -> Bitboard {
        self.checked_add_vector_const(directions)
    }
}
