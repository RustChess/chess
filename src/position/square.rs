use std::{fmt, str::FromStr};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid square: {0}")]
    Invalid(String),
}

crate::finite_set!(
    File,
    FileTable {
        A = 0,
        B = 1,
        C = 2,
        D = 3,
        E = 4,
        F = 5,
        G = 6,
        H = 7,
    }
);

impl File {
    pub const fn from_char(c: char) -> Option<Self> {
        if ('a' <= c && c <= 'h') || ('A' <= c && c <= 'H') {
            Some(Self::panicky_from_char(c))
        } else {
            None
        }
    }

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
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        f.write_char(self.lower())
    }
}

crate::finite_set!(
    Rank,
    RankTable {
        One = 0,
        Two = 1,
        Three = 2,
        Four = 3,
        Five = 4,
        Six = 5,
        Seven = 6,
        Eight = 7,
    }
);

impl Rank {
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
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        f.write_char(self.char())
    }
}

crate::finite_set!(
    /// The squares of a chess board.
    Square,
    SquareTable {
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
);

impl Square {
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
