use std::{fmt, str::FromStr};

use super::All;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid square: {0}")]
    Invalid(String),
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum File {
    A = 0,
    B = 1,
    C = 2,
    D = 3,
    E = 4,
    F = 5,
    G = 6,
    H = 7,
}

impl File {
    pub const ALL: [File; 8] =
        [File::A, File::B, File::C, File::D, File::E, File::F, File::G, File::H];

    pub const fn index_const(self) -> usize {
        self as u8 as usize
    }

    #[track_caller]
    #[inline]
    pub const fn from_index(index: u8) -> Option<Self> {
        let within_bounds = index < 8;
        if within_bounds { Some(Self::panicky_from_index(index)) } else { None }
    }

    #[track_caller]
    #[inline]
    pub(crate) const fn panicky_from_index(index: u8) -> File {
        assert!(index < 8);
        unsafe { core::mem::transmute(index) }
    }

    pub const fn from_char(c: char) -> Option<Self> {
        if 'a' <= c && c <= 'h' { Some(Self::panicky_from_char(c)) } else { None }
    }

    #[track_caller]
    pub(crate) const fn panicky_from_char(c: char) -> Self {
        assert!('a' <= c && c <= 'h');
        unsafe { core::mem::transmute(c as u8 - b'a') }
    }

    #[inline]
    pub const fn lower(self) -> char {
        use File::*;
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

    pub fn iter() -> impl Iterator<Item = Self> {
        (0..8).map(Self::panicky_from_index)
    }

    pub fn iter_rev() -> impl Iterator<Item = Self> {
        (0..8).rev().map(Self::panicky_from_index)
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        f.write_char(self.lower())
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Rank {
    One = 0,
    Two = 1,
    Three = 2,
    Four = 3,
    Five = 4,
    Six = 5,
    Seven = 6,
    Eight = 7,
}

impl Rank {
    pub const ALL: [Rank; 8] = [
        Rank::One,
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
    ];

    pub const fn index_const(self) -> usize {
        self as u8 as usize
    }

    #[track_caller]
    #[inline]
    pub const fn from_index(index: u8) -> Option<Self> {
        let within_bounds = index < 8;
        if within_bounds { Some(Self::panicky_from_index(index)) } else { None }
    }

    #[track_caller]
    #[inline]
    pub(crate) const fn panicky_from_index(index: u8) -> Rank {
        assert!(index < 8);
        unsafe { core::mem::transmute(index) }
    }

    pub const fn from_char(c: char) -> Option<Self> {
        if '1' <= c && c <= '8' { Some(Self::panicky_from_char(c)) } else { None }
    }

    #[track_caller]
    pub(crate) const fn panicky_from_char(c: char) -> Self {
        assert!('1' <= c && c <= '8');
        unsafe { core::mem::transmute(c as u8 - b'1') }
    }

    #[inline]
    pub const fn char(self) -> char {
        use Rank::*;
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

    pub fn iter() -> impl Iterator<Item = Self> {
        (0..8).map(Self::panicky_from_index)
    }

    pub fn iter_rev() -> impl Iterator<Item = Self> {
        (0..8).rev().map(Self::panicky_from_index)
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        f.write_char(self.char())
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(SerializeDisplay))]
pub enum Square {
    A1 = 0,
    B1,
    C1,
    D1,
    E1,
    F1,
    G1,
    H1,
    A2 = 8,
    B2,
    C2,
    D2,
    E2,
    F2,
    G2,
    H2,
    A3 = 16,
    B3,
    C3,
    D3,
    E3,
    F3,
    G3,
    H3,
    A4 = 24,
    B4,
    C4,
    D4,
    E4,
    F4,
    G4,
    H4,
    A5 = 32,
    B5,
    C5,
    D5,
    E5,
    F5,
    G5,
    H5,
    A6 = 40,
    B6,
    C6,
    D6,
    E6,
    F6,
    G6,
    H6,
    A7 = 48,
    B7,
    C7,
    D7,
    E7,
    F7,
    G7,
    H7,
    A8 = 56,
    B8,
    C8,
    D8,
    E8,
    F8,
    G8,
    H8,
}

impl Square {
    pub const fn index_const(self) -> usize {
        self as u8 as usize
    }
}

impl All<64> for Square {
    const ALL: [Square; 64] = [
        Square::A1,
        Square::B1,
        Square::C1,
        Square::D1,
        Square::E1,
        Square::F1,
        Square::G1,
        Square::H1,
        Square::A2,
        Square::B2,
        Square::C2,
        Square::D2,
        Square::E2,
        Square::F2,
        Square::G2,
        Square::H2,
        Square::A3,
        Square::B3,
        Square::C3,
        Square::D3,
        Square::E3,
        Square::F3,
        Square::G3,
        Square::H3,
        Square::A4,
        Square::B4,
        Square::C4,
        Square::D4,
        Square::E4,
        Square::F4,
        Square::G4,
        Square::H4,
        Square::A5,
        Square::B5,
        Square::C5,
        Square::D5,
        Square::E5,
        Square::F5,
        Square::G5,
        Square::H5,
        Square::A6,
        Square::B6,
        Square::C6,
        Square::D6,
        Square::E6,
        Square::F6,
        Square::G6,
        Square::H6,
        Square::A7,
        Square::B7,
        Square::C7,
        Square::D7,
        Square::E7,
        Square::F7,
        Square::G7,
        Square::H7,
        Square::A8,
        Square::B8,
        Square::C8,
        Square::D8,
        Square::E8,
        Square::F8,
        Square::G8,
        Square::H8,
    ];

    fn index(self) -> usize {
        self.index_const()
    }
}

impl Square {
    pub const ALL: [Square; 64] = <Square as All<64>>::ALL;

    pub const fn new(file: File, rank: Rank) -> Self {
        Self::panicky_from_index(((rank as u8) << 3) | (file as u8))
    }

    /// A1, B1, ..., H8.
    pub fn iter() -> impl Iterator<Item = Self> {
        (0..64).map(Self::panicky_from_index)
    }

    /// A8, B8, ..., H8, A7, ..., H1.
    pub fn rank_rev_iter() -> impl Iterator<Item = Square> {
        Rank::iter_rev().flat_map(|rank| File::iter().map(move |file| Square::new(file, rank)))
    }

    #[track_caller]
    #[inline]
    pub const fn from_index(index: u8) -> Option<Self> {
        if index < 64 { Some(Self::panicky_from_index(index)) } else { None }
    }

    #[track_caller]
    #[inline]
    pub(crate) const fn panicky_from_index(index: u8) -> Square {
        assert!(index < 64);
        unsafe { core::mem::transmute(index) }
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
}

impl FromStr for Square {
    type Err = Error;

    fn from_str(square: &str) -> Result<Self> {
        let mut chars = square.chars();

        let Some(file) = chars.next().and_then(File::from_char) else {
            return Err(Error::Invalid(square.to_string()));
        };
        let Some(rank) = chars.next().and_then(Rank::from_char) else {
            return Err(Error::Invalid(square.to_string()));
        };
        if chars.next().is_some() {
            return Err(Error::Invalid(square.to_string()));
        }

        Ok(Square::new(file, rank))
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        f.write_char(self.file().lower())?;
        f.write_char(self.rank().char())
    }
}

#[test]
fn display_square() {
    assert_eq!(Square::A7.to_string(), "a7".to_string());
    assert_eq!(Square::B3.to_string(), "b3".to_string());
    assert_eq!("e4".parse::<Square>().unwrap(), Square::E4);
}
