use core::str::FromStr;

use crate::board::Bitboard;

use File::*;
use Rank::*;

mod geometry;
mod moves;

pub use geometry::Direction;
pub use moves::SliderSights;

crate::finite_set!(
    File,
    FileTable {
        A = 0 as a,
        B = 1 as b,
        C = 2 as c,
        D = 3 as d,
        E = 4 as e,
        F = 5 as f,
        G = 6 as g,
        H = 7 as h,
    },
    FileCursor
);

crate::finite_set!(
    Rank,
    RankTable {
        One = 0 as "1",
        Two = 1 as "2",
        Three = 2 as "3",
        Four = 3 as "4",
        Five = 4 as "5",
        Six = 5 as "6",
        Seven = 6 as "7",
        Eight = 7 as "8",
    }
);

crate::finite_set!(
    /// The squares of a chess board.
    Square,
    SquareTable {
        A1 = 0 as a1,
        B1 = 1 as b1,
        C1 = 2 as c1,
        D1 = 3 as d1,
        E1 = 4 as e1,
        F1 = 5 as f1,
        G1 = 6 as g1,
        H1 = 7 as h1,
        A2 = 8 as a2,
        B2 = 9 as b2,
        C2 = 10 as c2,
        D2 = 11 as d2,
        E2 = 12 as e2,
        F2 = 13 as f2,
        G2 = 14 as g2,
        H2 = 15 as h2,
        A3 = 16 as a3,
        B3 = 17 as b3,
        C3 = 18 as c3,
        D3 = 19 as d3,
        E3 = 20 as e3,
        F3 = 21 as f3,
        G3 = 22 as g3,
        H3 = 23 as h3,
        A4 = 24 as a4,
        B4 = 25 as b4,
        C4 = 26 as c4,
        D4 = 27 as d4,
        E4 = 28 as e4,
        F4 = 29 as f4,
        G4 = 30 as g4,
        H4 = 31 as h4,
        A5 = 32 as a5,
        B5 = 33 as b5,
        C5 = 34 as c5,
        D5 = 35 as d5,
        E5 = 36 as e5,
        F5 = 37 as f5,
        G5 = 38 as g5,
        H5 = 39 as h5,
        A6 = 40 as a6,
        B6 = 41 as b6,
        C6 = 42 as c6,
        D6 = 43 as d6,
        E6 = 44 as e6,
        F6 = 45 as f6,
        G6 = 46 as g6,
        H6 = 47 as h6,
        A7 = 48 as a7,
        B7 = 49 as b7,
        C7 = 50 as c7,
        D7 = 51 as d7,
        E7 = 52 as e7,
        F7 = 53 as f7,
        G7 = 54 as g7,
        H7 = 55 as h7,
        A8 = 56 as a8,
        B8 = 57 as b8,
        C8 = 58 as c8,
        D8 = 59 as d8,
        E8 = 60 as e8,
        F8 = 61 as f8,
        G8 = 62 as g8,
        H8 = 63 as h8,
    }
);

impl File {
    #[inline]
    pub const fn from_char(c: char) -> Option<Self> {
        if ('a' <= c && c <= 'h') || ('A' <= c && c <= 'H') {
            Some(Self::panicky_from_char(c))
        } else {
            None
        }
    }

    #[inline]
    #[track_caller]
    pub(crate) const fn panicky_from_char(c: char) -> Self {
        let index = if 'a' <= c && c <= 'h' {
            c as u8 - b'a'
        } else {
            assert!('A' <= c && c <= 'H');
            c as u8 - b'A'
        };
        unsafe { core::mem::transmute(index) }
    }

    #[inline]
    pub const fn lower(self) -> char {
        match self {
            A => 'a',
            B => 'b',
            C => 'c',
            D => 'd',
            E => 'e',
            F => 'f',
            G => 'g',
            H => 'h',
        }
    }

    #[inline]
    pub const fn upper(self) -> char {
        match self {
            A => 'A',
            B => 'B',
            C => 'C',
            D => 'D',
            E => 'E',
            F => 'F',
            G => 'G',
            H => 'H',
        }
    }
}

impl Rank {
    #[inline]
    pub const fn from_char(c: char) -> Option<Self> {
        if '1' <= c && c <= '8' { Some(Self::panicky_from_char(c)) } else { None }
    }

    #[track_caller]
    #[inline]
    pub(crate) const fn panicky_from_char(c: char) -> Self {
        assert!('1' <= c && c <= '8');
        unsafe { core::mem::transmute(c as u8 - b'1') }
    }

    #[inline]
    pub const fn char(self) -> char {
        match self {
            One => '1',
            Two => '2',
            Three => '3',
            Four => '4',
            Five => '5',
            Six => '6',
            Seven => '7',
            Eight => '8',
        }
    }
}

impl Square {
    #[inline]
    pub const fn new(file: File, rank: Rank) -> Self {
        Self::panicky_from_index(((rank as u8) << 3) | (file as u8))
    }

    /// A8, B8, ..., H8, A7, ..., H1.
    pub fn rank_rev_iter() -> impl Iterator<Item = Square> {
        Rank::iter_rev().flat_map(|rank| File::iter().map(move |file| Square::new(file, rank)))
    }

    #[inline]
    pub const fn file(self) -> File {
        File::panicky_from_index((self as u8) & 0x7)
    }

    #[inline]
    pub const fn rank(self) -> Rank {
        Rank::panicky_from_index((self as u8) >> 3)
    }

    #[inline]
    pub const fn coordinates(self) -> (File, Rank) {
        (self.file(), self.rank())
    }

    // Add step vector to square, union into a bitboard.
    const fn checked_add_vector_const(self, directions: &[Direction]) -> Bitboard {
        let mut attacks = Bitboard::EMPTY;
        let mut i = 0;
        while i < directions.len() {
            if let Some(target) = self.checked_add(directions[i]) {
                attacks.append(Bitboard::from_square(target));
            }
            i += 1;
        }
        attacks
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SquareError {
    #[error("invalid square: {0}")]
    Invalid(String),
}

impl FromStr for Square {
    type Err = SquareError;

    fn from_str(square: &str) -> Result<Self, Self::Err> {
        let mut chars = square.chars();

        let Some(file) = chars.next().and_then(File::from_char) else {
            return Err(SquareError::Invalid(square.to_string()));
        };
        let Some(rank) = chars.next().and_then(Rank::from_char) else {
            return Err(SquareError::Invalid(square.to_string()));
        };
        if chars.next().is_some() {
            return Err(SquareError::Invalid(square.to_string()));
        }

        Ok(Square::new(file, rank))
    }
}

#[test]
fn display_square() {
    assert_eq!(Square::A7.to_string(), "a7".to_string());
    assert_eq!(Square::B3.to_string(), "b3".to_string());
    assert_eq!("e4".parse::<Square>().unwrap(), Square::E4);
}
