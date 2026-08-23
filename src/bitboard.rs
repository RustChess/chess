// TODO: It feels like it would be more logical to
// rename all the *_const as just *, and have the caller
// do the `.into()` calls

use core::{fmt, ops};

use super::position::{File, Rank, Square};

#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Bitboard(pub u64);

#[derive(Clone, Copy)]
#[repr(i8)]
// This is an enum and not e.g. Direction(i8) for two reasons
// - implementing shift
// - matching in const contexts
pub enum Direction {
    North = direction(0, 1),
    East = direction(1, 0),
    South = direction(0, -1),
    West = direction(-1, 0),
    NorthNorth = direction(0, 2),
    SouthSouth = direction(0, -2),

    NorthEast = direction(1, 1),
    NorthWest = direction(-1, 1),
    SouthEast = direction(1, -1),
    SouthWest = direction(-1, -1),

    KnightNorthEast = direction(1, 2),
    KnightNorthWest = direction(-1, 2),
    KnightEastNorth = direction(2, 1),
    KnightEastSouth = direction(2, -1),
    KnightSouthEast = direction(1, -2),
    KnightSouthWest = direction(-1, -2),
    KnightWestNorth = direction(-2, 1),
    KnightWestSouth = direction(-2, -1),
}

const fn direction(file: i8, rank: i8) -> i8 {
    rank * 8 + file
}

const RANKS: [u64; 8] = {
    let mut masks = [0; 8];
    let mut i = 0;
    while i < 8 {
        masks[i] = 0xff << (i * 8);
        i += 1;
    }
    masks
};

const fn full_rays() -> [[Bitboard; 64]; 64] {
    let mut table = [[Bitboard::EMPTY; 64]; 64];
    let mut a = 0i32;
    while a < 64 {
        let file_a = a & 7;
        let rank_a = a >> 3;
        let diagonal_a = rank_a - file_a;
        let antidiagonal_a = rank_a + file_a - 7;

        let mut b = 0i32;
        while b < 64 {
            let file_b = b & 7;
            let rank_b = b >> 3;
            let diagonal_b = rank_b - file_b;
            let antidiagonal_b = rank_b + file_b - 7;

            table[a as usize][b as usize] = Bitboard(if a == b {
                0
            } else if file_a == file_b {
                Bitboard::FILE_A.0 << file_a
            } else if rank_a == rank_b {
                0xff << (8 * rank_a)
            } else if diagonal_a == diagonal_b {
                if diagonal_a >= 0 {
                    Bitboard::DIAGONAL.0 << (8 * diagonal_a)
                } else {
                    Bitboard::DIAGONAL.0 >> (8 * -diagonal_a)
                }
            } else if antidiagonal_a == antidiagonal_b {
                if antidiagonal_a >= 0 {
                    Bitboard::ANTIDIAGONAL.0 << (8 * antidiagonal_a)
                } else {
                    Bitboard::ANTIDIAGONAL.0 >> (8 * -antidiagonal_a)
                }
            } else {
                0
            });
            b += 1;
        }
        a += 1;
    }
    table
}

#[test]
fn bitboard_constants() {
    println!("{:?}", Bitboard::NORTH);
    println!("{:?}", Bitboard::EAST);
    println!("{:?}", Bitboard::SOUTH);
    println!("{:?}", Bitboard::WEST);
}

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const FULL: Bitboard = Bitboard(!0);
    pub const BLACK: Bitboard = Bitboard(0xaa55_aa55_aa55_aa55);
    pub const WHITE: Bitboard = Bitboard(0x55aa_55aa_55aa_55aa);
    pub const CORNERS: Bitboard = Bitboard(0x8100_0000_0000_0081);
    pub const BACKRANKS: Bitboard = Bitboard(0xff00_0000_0000_00ff);
    pub const CENTER: Bitboard = Bitboard(0x0000_0018_1800_0000);
    pub const NORTH: Bitboard = Bitboard(0xffff_ffff_0000_0000);
    pub const SOUTH: Bitboard = Bitboard(0x0000_0000_ffff_ffff);
    pub const WEST: Bitboard = Bitboard(0x0f0f_0f0f_0f0f_0f0f);
    pub const EAST: Bitboard = Bitboard(0xf0f0_f0f0_f0f0_f0f0);
    pub const FILE_A: Bitboard = Bitboard(0x0101_0101_0101_0101);
    // Full a1-h8 diagonal.
    pub const DIAGONAL: Bitboard = Bitboard(0x8040_2010_0804_0201);
    // Full h1-a8 antidiagonal.
    pub const ANTIDIAGONAL: Bitboard = Bitboard(0x0102_0408_1020_4080);
    pub const FULL_RAYS: [[Bitboard; 64]; 64] = full_rays();

    #[inline]
    pub const fn from_square(square: Square) -> Bitboard {
        Bitboard(1 << square as u32)
    }

    pub const fn from_squares<const N: usize>(squares: [Square; N]) -> Bitboard {
        let mut bitboard = Bitboard::EMPTY;
        let mut i = 0;
        while i < N {
            bitboard.append_const(Bitboard::from_square(squares[i]));
            i += 1;
        }
        bitboard
    }

    #[inline]
    pub const fn from_file(file: File) -> Bitboard {
        Bitboard(Bitboard::FILE_A.0 << file as u32)
    }

    #[inline]
    pub const fn from_rank(rank: Rank) -> Bitboard {
        Bitboard(RANKS[rank as usize])
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn eq_const(self, other: Self) -> bool {
        self.0 == other.0
    }

    #[inline]
    pub const fn contains(self, square: Square) -> bool {
        !self.intersection_const(Bitboard::from_square(square)).is_empty()
    }

    #[inline]
    pub const fn with(self, square: Square) -> Bitboard {
        self.union_const(Bitboard::from_square(square))
    }

    #[inline]
    // Non-standard
    pub const fn set(&mut self, square: Square, set: bool) {
        if set {
            self.append_const(Bitboard::from_square(square));
        } else {
            *self = self.difference_const(Bitboard::from_square(square));
        }
    }

    #[inline]
    pub const fn insert(&mut self, square: Square) -> bool {
        if self.contains(square) {
            false
        } else {
            self.append_const(Bitboard::from_square(square));
            true
        }
    }

    #[inline]
    pub const fn remove(&mut self, square: Square) -> bool {
        if self.contains(square) {
            *self = self.difference_const(Bitboard::from_square(square));
            true
        } else {
            false
        }
    }

    #[inline]
    pub const fn clear(&mut self) {
        self.0 = 0;
    }

    pub const fn without_a_file(self) -> Bitboard {
        self.difference_const(Bitboard::FILE_A)
    }

    pub const fn without_h_file(self) -> Bitboard {
        self.difference_const(Bitboard::from_file(File::H))
    }

    pub const fn without_ab_files(self) -> Bitboard {
        self.without_a_file().difference_const(Bitboard::from_file(File::B))
    }

    pub const fn without_gh_files(self) -> Bitboard {
        self.without_h_file().difference_const(Bitboard::from_file(File::G))
    }

    /// Appends `squares`.
    #[inline]
    pub fn append<T: Into<Bitboard>>(&mut self, squares: T) {
        self.append_const(squares.into());
    }

    #[inline]
    pub const fn append_const(&mut self, squares: Bitboard) {
        self.0 |= squares.0;
    }

    #[inline]
    pub fn difference<T: Into<Bitboard>>(&self, squares: T) -> Bitboard {
        self.difference_const(squares.into())
    }

    #[inline]
    pub const fn difference_const(&self, squares: Bitboard) -> Bitboard {
        Bitboard(self.0 & !squares.0)
    }

    #[inline]
    pub fn intersection<T: Into<Bitboard>>(self, squares: T) -> Bitboard {
        self.intersection_const(squares.into())
    }

    #[inline]
    pub const fn intersection_const(self, squares: Bitboard) -> Bitboard {
        Bitboard(self.0 & squares.0)
    }

    #[inline]
    pub fn symmetric_difference<T: Into<Bitboard>>(self, squares: T) -> Bitboard {
        self.symmetric_difference_const(squares.into())
    }

    #[inline]
    pub const fn symmetric_difference_const(self, squares: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ squares.0)
    }

    #[inline]
    pub fn is_disjoint<T: Into<Bitboard>>(self, other: T) -> bool {
        self.is_disjoint_const(other.into())
    }

    #[inline]
    pub const fn is_disjoint_const(self, other: Bitboard) -> bool {
        self.intersection_const(other).is_empty()
    }

    #[inline]
    pub fn is_subset<T: Into<Bitboard>>(self, other: T) -> bool {
        self.is_subset_const(other.into())
    }

    pub const fn is_subset_const(self, other: Bitboard) -> bool {
        self.difference_const(other).is_empty()
    }

    pub fn is_superset<T: Into<Bitboard>>(self, other: T) -> bool {
        self.is_superset_const(other.into())
    }

    pub const fn is_superset_const(self, other: Bitboard) -> bool {
        other.is_subset_const(self)
    }

    #[inline]
    pub const fn first(self) -> Option<Square> {
        if self.is_empty() {
            None
        } else {
            Some(Square::panicky_new(self.0.trailing_zeros() as u8))
        }
    }

    #[inline]
    pub const fn pop_first(&mut self) -> Option<Square> {
        let square = self.first();
        self.discard_first();
        square
    }

    #[inline]
    const fn discard_first(&mut self) {
        *self = self.without_first();
    }

    #[inline]
    pub const fn without_first(self) -> Bitboard {
        let Bitboard(mask) = self;
        Bitboard(mask & mask.wrapping_sub(1))
    }

    #[inline]
    pub const fn more_than_one(self) -> bool {
        !self.without_first().is_empty()
    }

    #[inline]
    pub const fn pop_last(&mut self) -> Option<Square> {
        let square = self.last();
        self.discard_last();
        square
    }

    #[inline]
    pub const fn last(self) -> Option<Square> {
        if let Some(index) = self.0.checked_ilog2() {
            Some(Square::panicky_new(index as u8))
        } else {
            None
        }
    }

    #[inline]
    const fn discard_last(&mut self) {
        *self = self.without_last();
    }

    #[inline]
    const fn without_last(self) -> Bitboard {
        let Bitboard(mask) = self;
        Bitboard(mask & !((1u64 << 63).wrapping_shr(mask.leading_zeros())))
    }

    #[inline]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    #[inline]
    pub fn union<T: Into<Bitboard>>(self, squares: T) -> Bitboard {
        self.union_const(squares.into())
    }

    #[inline]
    pub const fn union_const(self, squares: Bitboard) -> Bitboard {
        Bitboard(self.0 | squares.0)
    }

    #[inline]
    pub const fn checked_shift(self, direction: Direction) -> Bitboard {
        use Direction::*;

        match direction {
            North => self.wrapping_shift(North),
            South => self.wrapping_shift(South),
            East => self.without_h_file().wrapping_shift(East),
            West => self.without_a_file().wrapping_shift(West),
            NorthNorth => self.wrapping_shift(NorthNorth),
            SouthSouth => self.wrapping_shift(SouthSouth),
            NorthWest => self.without_a_file().wrapping_shift(NorthWest),
            SouthWest => self.without_a_file().wrapping_shift(SouthWest),
            NorthEast => self.without_h_file().wrapping_shift(NorthEast),
            SouthEast => self.without_h_file().wrapping_shift(SouthEast),
            KnightNorthWest => self.without_a_file().wrapping_shift(KnightNorthWest),
            KnightSouthWest => self.without_a_file().wrapping_shift(KnightSouthWest),
            KnightNorthEast => self.without_h_file().wrapping_shift(KnightNorthEast),
            KnightSouthEast => self.without_h_file().wrapping_shift(KnightSouthEast),
            KnightWestNorth => self.without_ab_files().wrapping_shift(KnightWestNorth),
            KnightWestSouth => self.without_ab_files().wrapping_shift(KnightWestSouth),
            KnightEastNorth => self.without_gh_files().wrapping_shift(KnightEastNorth),
            KnightEastSouth => self.without_gh_files().wrapping_shift(KnightEastSouth),
        }
    }

    const fn wrapping_shift(self, direction: Direction) -> Bitboard {
        let offset = direction as i8;
        if offset >= 0 {
            Bitboard(self.0 << offset as u32)
        } else {
            Bitboard(self.0 >> -offset as u32)
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = Square> {
        self.into_iter()
    }

    #[inline]
    // Non-standard
    pub const fn flip_vertical(self) -> Bitboard {
        Bitboard(self.0.swap_bytes())
    }

    #[inline]
    // Non-standard
    pub const fn flip_horizontal(self) -> Bitboard {
        // https://www.chessprogramming.org/Flipping_Mirroring_and_Rotating#Horizontal
        let k1 = 0x5555_5555_5555_5555;
        let k2 = 0x3333_3333_3333_3333;
        let k4 = 0x0f0f_0f0f_0f0f_0f0f;
        let x = self.0;
        let x = ((x >> 1) & k1) | ((x & k1) << 1);
        let x = ((x >> 2) & k2) | ((x & k2) << 2);
        let x = ((x >> 4) & k4) | ((x & k4) << 4);
        Bitboard(x)
    }

    #[inline]
    /// Mirror at a1-h8 diagonal
    pub const fn flip_diagonal(self) -> Bitboard {
        // https://www.chessprogramming.org/Flipping_Mirroring_and_Rotating#Diagonal
        let k1 = 0x5500_5500_5500_5500;
        let k2 = 0x3333_0000_3333_0000;
        let k4 = 0x0f0f_0f0f_0000_0000;
        let mut x = self.0;
        let t = k4 & (x ^ (x << 28));
        x ^= t ^ (t >> 28);
        let t = k2 & (x ^ (x << 14));
        x ^= t ^ (t >> 14);
        let t = k1 & (x ^ (x << 7));
        x ^= t ^ (t >> 7);
        Bitboard(x)
    }

    #[inline]
    /// Mirror at h1-a8 diagonal
    pub const fn flip_anti_diagonal(self) -> Bitboard {
        // https://www.chessprogramming.org/Flipping_Mirroring_and_Rotating#Anti-Diagonal
        let k1 = 0xaa00_aa00_aa00_aa00;
        let k2 = 0xcccc_0000_cccc_0000;
        let k4 = 0xf0f0_f0f0_0f0f_0f0f;
        let mut x = self.0;
        let t = x ^ (x << 36);
        x ^= k4 & (t ^ (x >> 36));
        let t = k2 & (x ^ (x << 18));
        x ^= t ^ (t >> 18);
        let t = k1 & (x ^ (x << 9));
        x ^= t ^ (t >> 9);
        Bitboard(x)
    }

    #[inline]
    // Rotate 90 degrees clockwise
    pub const fn rotate_90(self) -> Bitboard {
        self.flip_diagonal().flip_vertical()
    }

    #[inline]
    // Rotate 180 degrees (clockwise)
    pub const fn rotate_180(self) -> Bitboard {
        Bitboard(self.0.reverse_bits())
    }

    // Rotate 270 degrees clockwise
    pub const fn rotate_270(self) -> Bitboard {
        self.flip_vertical().flip_diagonal()
    }
}

impl Direction {
    #[inline]
    pub const fn reverse(self) -> Direction {
        use Direction::*;

        match self {
            North => South,
            East => West,
            South => North,
            West => East,
            NorthNorth => SouthSouth,
            SouthSouth => NorthNorth,
            NorthEast => SouthWest,
            NorthWest => SouthEast,
            SouthEast => NorthWest,
            SouthWest => NorthEast,
            KnightNorthEast => KnightSouthWest,
            KnightNorthWest => KnightSouthEast,
            KnightEastNorth => KnightWestSouth,
            KnightEastSouth => KnightWestNorth,
            KnightSouthEast => KnightNorthWest,
            KnightSouthWest => KnightNorthEast,
            KnightWestNorth => KnightEastSouth,
            KnightWestSouth => KnightEastNorth,
        }
    }
}

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        for rank in Rank::iter_rev() {
            for file in File::iter() {
                let square = Square::new(file, rank);
                f.write_char(if self.contains(square) { '1' } else { '.' })?;
                f.write_char(if file < File::H { ' ' } else { '\n' })?;
            }
        }

        Ok(())
    }
}

impl fmt::UpperHex for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}

impl fmt::LowerHex for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::Octal for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Octal::fmt(&self.0, f)
    }
}

impl fmt::Binary for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Binary::fmt(&self.0, f)
    }
}

impl From<Square> for Bitboard {
    #[inline]
    fn from(sq: Square) -> Bitboard {
        Bitboard::from_square(sq)
    }
}

impl From<Rank> for Bitboard {
    #[inline]
    fn from(rank: Rank) -> Bitboard {
        Bitboard::from_rank(rank)
    }
}

impl From<File> for Bitboard {
    #[inline]
    fn from(file: File) -> Bitboard {
        Bitboard::from_file(file)
    }
}

impl From<u64> for Bitboard {
    #[inline]
    fn from(bitboard: u64) -> Bitboard {
        Bitboard(bitboard)
    }
}

impl From<Bitboard> for u64 {
    #[inline]
    fn from(bitboard: Bitboard) -> u64 {
        bitboard.0
    }
}

impl<T> ops::BitAnd<T> for Bitboard
where
    T: Into<Bitboard>,
{
    type Output = Bitboard;

    #[inline]
    fn bitand(self, rhs: T) -> Bitboard {
        let Bitboard(rhs) = rhs.into();
        Bitboard(self.0 & rhs)
    }
}

impl<T> ops::BitAndAssign<T> for Bitboard
where
    T: Into<Bitboard>,
{
    #[inline]
    fn bitand_assign(&mut self, rhs: T) {
        let Bitboard(rhs) = rhs.into();
        self.0 &= rhs;
    }
}

impl<T> ops::BitOr<T> for Bitboard
where
    T: Into<Bitboard>,
{
    type Output = Bitboard;

    #[inline]
    fn bitor(self, rhs: T) -> Bitboard {
        let Bitboard(rhs) = rhs.into();
        Bitboard(self.0 | rhs)
    }
}

impl<T> ops::BitOrAssign<T> for Bitboard
where
    T: Into<Bitboard>,
{
    #[inline]
    fn bitor_assign(&mut self, rhs: T) {
        let Bitboard(rhs) = rhs.into();
        self.0 |= rhs;
    }
}

impl<T> ops::BitXor<T> for Bitboard
where
    T: Into<Bitboard>,
{
    type Output = Bitboard;

    #[inline]
    fn bitxor(self, rhs: T) -> Bitboard {
        let Bitboard(rhs) = rhs.into();
        Bitboard(self.0 ^ rhs)
    }
}

impl<T> ops::BitXorAssign<T> for Bitboard
where
    T: Into<Bitboard>,
{
    #[inline]
    fn bitxor_assign(&mut self, rhs: T) {
        let Bitboard(rhs) = rhs.into();
        self.0 ^= rhs;
    }
}

impl ops::Not for Bitboard {
    type Output = Bitboard;

    #[inline]
    fn not(self) -> Bitboard {
        Bitboard(!self.0)
    }
}

impl FromIterator<Square> for Bitboard {
    fn from_iter<T>(iter: T) -> Bitboard
    where
        T: IntoIterator<Item = Square>,
    {
        let mut bitboard = Bitboard(0);
        bitboard.extend(iter);
        bitboard
    }
}

impl Extend<Square> for Bitboard {
    fn extend<T: IntoIterator<Item = Square>>(&mut self, iter: T) {
        for square in iter {
            self.append(square);
        }
    }
}

impl IntoIterator for Bitboard {
    type Item = Square;
    type IntoIter = IntoIter;

    #[inline]
    fn into_iter(self) -> IntoIter {
        IntoIter(self)
    }
}

/// Iterator over the squares of a [`Bitboard`].
#[derive(Clone, Debug, Default)]
pub struct IntoIter(Bitboard);

impl Iterator for IntoIter {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Square> {
        self.0.pop_first()
    }

    #[inline]
    fn count(self) -> usize {
        self.0.len()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.0.len();
        (len, Some(len))
    }

    #[inline]
    fn last(self) -> Option<Square> {
        self.0.last()
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Square) -> B,
    {
        let mut accum = init;
        let Bitboard(mut mask) = self.0;
        while mask != 0 {
            accum = f(accum, Square::panicky_new(mask.trailing_zeros() as u8));
            mask = mask & mask.wrapping_sub(1);
        }
        accum
    }
}

impl ExactSizeIterator for IntoIter {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl DoubleEndedIterator for IntoIter {
    #[inline]
    fn next_back(&mut self) -> Option<Square> {
        self.0.pop_last()
    }
}

impl core::iter::FusedIterator for IntoIter {}
