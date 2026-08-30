use core::ops;

use crate::finite::{FiniteSet, Table};

/// Restricted square, can only be on third or sixth rank
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Square(super::Square);

pub type SquareTable<T = bool> = Table<Square, T, 16>;

impl TryFrom<super::Square> for Square {
    type Error = ();
    fn try_from(square: super::Square) -> Result<Self, ()> {
        Ok(match square as u8 {
            16..=24 => Self(square),
            40..=48 => Self(square),
            _ => todo!(),
        })
    }
}

impl ops::Deref for Square {
    type Target = super::Square;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Square> for super::Square {
    fn from(square: Square) -> super::Square {
        square.0
    }
}

impl FiniteSet<16> for Square {
    const ALL: [Square; 16] = Square::ALL;
}

impl Square {
    pub const fn index(self) -> usize {
        match self {
            Square(super::Square::A3) => 0,
            Square(super::Square::B3) => 1,
            Square(super::Square::C3) => 2,
            Square(super::Square::D3) => 3,
            Square(super::Square::E3) => 4,
            Square(super::Square::F3) => 5,
            Square(super::Square::G3) => 6,
            Square(super::Square::H3) => 7,
            Square(super::Square::A6) => 8,
            Square(super::Square::B6) => 9,
            Square(super::Square::C6) => 10,
            Square(super::Square::D6) => 11,
            Square(super::Square::E6) => 12,
            Square(super::Square::F6) => 13,
            Square(super::Square::G6) => 14,
            Square(super::Square::H6) => 15,
            _ => unreachable!(),
        }
    }

    pub const ALL: [Square; 16] = [
        Square(super::Square::A3),
        Square(super::Square::B3),
        Square(super::Square::C3),
        Square(super::Square::D3),
        Square(super::Square::E3),
        Square(super::Square::F3),
        Square(super::Square::G3),
        Square(super::Square::H3),
        Square(super::Square::A6),
        Square(super::Square::B6),
        Square(super::Square::C6),
        Square(super::Square::D6),
        Square(super::Square::E6),
        Square(super::Square::F6),
        Square(super::Square::G6),
        Square(super::Square::H6),
    ];

    #[inline]
    pub const fn square(self) -> super::Square {
        self.0
    }
}

impl<T> Table<Square, T, 16> {
    pub const fn get_ref(&self, key: Square) -> &T {
        &self.all[key.index()]
    }

    pub const fn get_mut(&mut self, key: Square) -> &mut T {
        &mut self.all[key.index()]
    }
}

impl<T: Copy> Table<Square, T, 16> {
    pub const fn get(&self, key: Square) -> T {
        self.all[key.index()]
    }
}

impl<T> ops::Index<Square> for Table<Square, T, 16> {
    type Output = T;

    fn index(&self, key: Square) -> &T {
        &self.all[key.index()]
    }
}

impl<T> ops::IndexMut<Square> for Table<Square, T, 16> {
    fn index_mut(&mut self, key: Square) -> &mut T {
        &mut self.all[key.index()]
    }
}

// #[repr(u8)]
// pub enum Square {
//     A3 = 16, B3, C3, D3, E3, F3, G3, H3,
//     // A4, B4, C4, D4, E4, F4, G4, H4,
//     // A5, B5, C5, D5, E5, F5, G5, H5,
//     A6 = 40, B6, C6, D6, E6, F6, G6, H6,
//     // A7, B7, C7, D7, E7, F7, G7, H7,
//     // A8, B8, C8, D8, E8, F8, G8, H8,
// }
