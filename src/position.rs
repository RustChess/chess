//! # Chess positions
//!
//! Any [`Position`] is determined by:
//! - the location of pieces on the [`Board`]
//! - parameters for the next turn (these are not visible from the board itself)
//!   - the [`Player`] whose turn it is
//!   - the possible castle [`Sides`]
//!   - the possible en-passant [`Square`] if any
//! - counter for "reversible" turns since last "non-reversible" (aka pawn moves + captures) [`Move`], aka "halfmoves"
//! - counter of full rounds, aka "fullmoves"
//!
//! We are only interested in "classical" Chess, but keep positions generic over
//! chess variants, and implement for "freestyle" (aka Fisher Random aka Chess960) chess - to exercise our generality mindedness
//!
//! Compared to `shakmaty` our position is a concrete `struct`
use core::{fmt, marker::PhantomData, num::NonZeroU32, ops};

use std::collections::BTreeMap;

use crate::bitboard::Bitboard;

#[cfg(feature = "serde")]
use serde::Serialize;

pub mod en_passant;
pub mod id;

pub use Kind::Normal;
pub use Player::*;
pub use Role::*;
pub use Special::*;

// Note: This is FIDE notation, even though "O-O" and "O-O-O" is prettier
// Note: PGN uses vowel O instead of number 0
pub const FIDE_SHORT_CASTLE: &str = "0-0";
pub const PGN_SHORT_CASTLE: &str = "O-O";
pub const FIDE_LONG_CASTLE: &str = "0-0-0";
pub const PGN_LONG_CASTLE: &str = "O-O-O";

pub const ALGEBRAIC_SHORT_CASTLE: &str = PGN_SHORT_CASTLE;
pub const ALGEBRAIC_LONG_CASTLE: &str = PGN_LONG_CASTLE;
pub const ALGEBRAIC_MOVE: char = '-';
pub const ALGEBRAIC_CAPTURE: char = 'x';

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(SerializeDisplay))]
pub enum Player {
    Black = 0,
    White = 1,
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Player::Black => write!(f, "black"),
            Player::White => write!(f, "white"),
        }
    }
}

impl Player {
    pub const ALL: [Player; 2] = [Player::Black, Player::White];

    pub const fn backrank(self) -> Rank {
        match self {
            Player::Black => Rank::Eight,
            Player::White => Rank::One,
        }
    }
}

// This has 1 + 2 players + 6 roles, so should be 9 * 8 = 72 bytes
//
// The `occupied` field is redundant, at which point it would be 64 bytes or 512 bits.
// Maybe this fits in AVX-512 registers?
//
// Invariant: players disjoint, roles disjoint, both union to same (=occupied)
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// Location of pieces on the board
pub struct Board {
    pub occupied: Bitboard,
    pub players: Players<Bitboard>,
    pub roles: Roles<Bitboard>,
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        for rank in Rank::ALL.into_iter().rev() {
            for file in File::ALL {
                let square = Square::new(file, rank);
                f.write_char(self.piece_at(square).map_or('.', Piece::char))?;
                f.write_char(if file < File::H { ' ' } else { '\n' })?;
            }
        }

        Ok(())
    }
}

impl Board {
    pub const fn standard() -> Board {
        Board {
            occupied: Bitboard(0xffff_0000_0000_ffff),
            players: Players { black: Bitboard(0xffff_0000_0000_0000), white: Bitboard(0xffff) },
            roles: Roles {
                pawn: Bitboard(0x00ff_0000_0000_ff00),
                knight: Bitboard(0x4200_0000_0000_0042),
                bishop: Bitboard(0x2400_0000_0000_0024),
                rook: Bitboard(0x8100_0000_0000_0081),
                queen: Bitboard(0x0800_0000_0000_0008),
                king: Bitboard(0x1000_0000_0000_0010),
            },
        }
    }

    pub const fn empty() -> Board {
        Board {
            players: Players { black: Bitboard::EMPTY, white: Bitboard::EMPTY },
            roles: Roles {
                pawn: Bitboard::EMPTY,
                knight: Bitboard::EMPTY,
                bishop: Bitboard::EMPTY,
                rook: Bitboard::EMPTY,
                queen: Bitboard::EMPTY,
                king: Bitboard::EMPTY,
            },
            occupied: Bitboard::EMPTY,
        }
    }

    pub const fn split(self) -> (Players<Bitboard>, Roles<Bitboard>) {
        (self.players, self.roles)
    }

    #[inline]
    pub const fn occupied(self) -> Bitboard {
        self.occupied
    }

    #[inline]
    pub const fn pawns(self) -> Bitboard {
        self.roles.pawn
    }

    #[inline]
    pub const fn knights(self) -> Bitboard {
        self.roles.knight
    }

    #[inline]
    pub const fn bishops(self) -> Bitboard {
        self.roles.bishop
    }

    #[inline]
    pub const fn rooks(self) -> Bitboard {
        self.roles.rook
    }

    #[inline]
    pub const fn queens(self) -> Bitboard {
        self.roles.queen
    }

    #[inline]
    pub const fn kings(self) -> Bitboard {
        self.roles.king
    }

    #[inline]
    pub const fn black(self) -> Bitboard {
        self.players.black
    }

    #[inline]
    pub const fn white(self) -> Bitboard {
        self.players.white
    }

    #[inline]
    pub const fn player(self, player: Player) -> Bitboard {
        *self.players.get(player)
    }

    #[inline]
    pub const fn role(self, role: Role) -> Bitboard {
        *self.roles.get(role)
    }

    /// Bishops, rooks and queens.
    #[inline]
    pub const fn sliders(self) -> Bitboard {
        let Roles { bishop, rook, queen, .. } = self.roles;
        bishop.symmetric_difference_const(rook).symmetric_difference_const(queen)
    }

    /// Pawns, knights and kings.
    #[inline]
    pub const fn steppers(self) -> Bitboard {
        let Roles { pawn, knight, king, .. } = self.roles;
        pawn.symmetric_difference_const(knight).symmetric_difference_const(king)
    }

    #[inline]
    pub const fn king_of(self, player: Player) -> Option<Square> {
        self.roles.king.intersection_const(self.player(player)).first()
    }

    #[inline]
    pub fn player_at(self, square: Square) -> Option<Player> {
        self.players.find(|player| player.contains(square))
    }

    #[inline]
    pub fn role_at(self, square: Square) -> Option<Role> {
        if self.occupied.contains(square) {
            Some(self.roles.find_or_king(|role| role.contains(square)))
        } else {
            // catch early
            None
        }
    }

    #[inline]
    pub fn piece_at(self, square: Square) -> Option<Piece> {
        self.player_at(square)
            .map(|player| self.roles.find_or_king(|role| role.contains(square)).of(player))
    }
}

// pub const INITIAL: Board = Board;

#[derive(Debug, thiserror::Error)]
#[error("error")]
pub struct Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub trait Variant: Copy + Sized {
    /// Validate position as being legal
    fn validate(position: Unvalidated) -> Result<Position<Self>>;
    /// Legal moves in this position
    fn moves(position: &Position<Self>) -> Moves;
}

pub mod variant {

    #[derive(Copy, Clone)]
    pub struct Chess;
    impl super::Variant for Chess {
        fn validate(_position: super::Unvalidated) -> Result<super::Position<Self>, super::Error> {
            todo!();
        }

        fn moves(_position: &super::Position<Self>) -> super::Moves {
            todo!();
        }
    }

    #[derive(Copy, Clone)]
    pub struct Freestyle;
    impl super::Variant for Freestyle {
        fn validate(_position: super::Unvalidated) -> Result<super::Position<Self>, super::Error> {
            todo!();
        }

        fn moves(_position: &super::Position<Self>) -> super::Moves {
            todo!();
        }
    }

    #[derive(Copy, Clone)]
    pub struct Unvalidated;
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

    #[track_caller]
    #[inline]
    pub(crate) const fn panicky_new(coordinate: u8) -> File {
        use File::*;
        match coordinate {
            0 => A,
            1 => B,
            2 => C,
            3 => D,
            4 => E,
            5 => F,
            6 => G,
            7 => H,
            _ => unreachable!(),
        }
    }

    pub(crate) fn panicky_from_char(c: char) -> Self {
        use File::*;
        match c {
            'a' => A,
            'b' => B,
            'c' => C,
            'd' => D,
            'e' => E,
            'f' => F,
            'g' => G,
            'h' => H,
            _ => unreachable!(),
        }
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
        (0..8).map(Self::panicky_new)
    }

    pub fn iter_rev() -> impl Iterator<Item = Self> {
        (0..8).rev().map(Self::panicky_new)
    }
}

impl fmt::Display for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        f.write_char(self.lower())
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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

    #[track_caller]
    #[inline]
    pub(crate) const fn panicky_new(coordinate: u8) -> Rank {
        use Rank::*;
        match coordinate {
            0 => One,
            1 => Two,
            2 => Three,
            3 => Four,
            4 => Five,
            5 => Six,
            6 => Seven,
            7 => Eight,
            _ => unreachable!(),
        }
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
        (0..8).map(Self::panicky_new)
    }

    pub fn iter_rev() -> impl Iterator<Item = Self> {
        (0..8).rev().map(Self::panicky_new)
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        f.write_char(self.char())
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
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
        self as u8 as usize
    }
}

pub struct SquareIter(u8);

impl Iterator for SquareIter {
    type Item = Square;

    fn next(&mut self) -> Option<Square> {
        if self.0 == 64 {
            None
        } else {
            let square = Square::panicky_new(self.0);
            self.0 += 1;
            Some(square)
        }
    }
}

impl Square {
    pub const fn new(file: File, rank: Rank) -> Self {
        Self::panicky_new(((rank as u8) << 3) | (file as u8))
    }

    pub const fn iter() -> SquareIter {
        SquareIter(0)
    }

    #[track_caller]
    #[inline]
    pub(crate) const fn panicky_new(index: u8) -> Square {
        assert!(index < 64);
        unsafe { core::mem::transmute(index) }
        // match index {
        //     0 => A1, 1 => A2, 2 => A3, 3 => A4, 4 => A5, 5 => A6, 6 => A7, 7 => A8,
        //     8 => B1, 9 => B2, 10 => B3, 11 => B4, 12 => B5, 13 => B6, 14 => B7, 15 => B8,
        //     16 => C1, 17 => C2, 18 => C3, 19 => C4, 20 => C5, 21 => C6, 22 => C7, 23 => C8,
        //     // 0 => D1, 1 => D2, 2 => D3, 3 => D4, 4 => D5, 5 => D6, 6 => D7, 7 => D8,
        //     // 0 => E1, 1 => E2, 2 => E3, 3 => E4, 4 => E5, 5 => E6, 6 => E7, 7 => E8,
        //     // 0 => F1, 1 => F2, 2 => F3, 3 => F4, 4 => F5, 5 => F6, 6 => F7, 7 => F8,
        //     // 0 => G1, 1 => G2, 2 => G3, 3 => G4, 4 => G5, 5 => G6, 6 => G7, 7 => G8,
        //     // 0 => H1, 1 => H2, 2 => H3, 3 => H4, 4 => H5, 5 => H6, 6 => H7, 7 => H8,
        // }
    }

    #[inline]
    pub const fn file(self) -> File {
        File::panicky_new((self as u8) & 0x7)
    }

    #[inline]
    pub const fn rank(self) -> Rank {
        Rank::panicky_new((self as u8) >> 3)
    }

    #[inline]
    pub const fn coordinates(self) -> (File, Rank) {
        (self.file(), self.rank())
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
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(SerializeDisplay))]
pub enum Role {
    Pawn = 1,
    Knight = 2,
    Bishop = 3,
    Rook = 5,
    Queen = 9,
    King = 4,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Role::*;
        write!(
            f,
            "{}",
            match self {
                Pawn => "pawn",
                Knight => "knight",
                Bishop => "bishop",
                Rook => "rook",
                Queen => "queen",
                King => "king",
            }
        )
    }
}

impl All<6> for Role {
    const ALL: [Role; 6] = Role::ALL;

    fn index(self) -> usize {
        match self {
            Pawn => 0,
            Knight => 1,
            Bishop => 2,
            Rook => 3,
            Queen => 4,
            King => 5,
        }
    }
}

impl Role {
    pub const ALL: [Role; 6] =
        [Role::Pawn, Role::Knight, Role::Bishop, Role::Rook, Role::Queen, Role::King];

    pub(crate) fn panicky_from_char(c: char) -> Self {
        use Role::*;
        let c = c.to_lowercase().next().unwrap();
        match c {
            'p' => Pawn,
            'n' => Knight,
            'b' => Bishop,
            'r' => Rook,
            'q' => Queen,
            'k' => King,
            _ => unreachable!(),
        }
    }

    /// `Bishop.of(White)`
    #[inline]
    pub const fn of(self, player: Player) -> Piece {
        Piece { player, role: self }
    }

    pub const fn lower(self) -> char {
        use Role::*;
        match self {
            Pawn => 'p',
            Knight => 'n',
            Bishop => 'b',
            Rook => 'r',
            Queen => 'q',
            King => 'k',
        }
    }

    pub const fn upper(self) -> char {
        use Role::*;
        match self {
            Pawn => 'P',
            Knight => 'N',
            Bishop => 'B',
            Rook => 'R',
            Queen => 'Q',
            King => 'K',
        }
    }

    pub const fn black(self) -> char {
        self.lower()
    }

    pub const fn white(self) -> char {
        self.upper()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Piece {
    pub player: Player,
    pub role: Role,
}

impl Piece {
    pub const fn char(self) -> char {
        match self.player {
            Player::Black => self.role.black(),
            Player::White => self.role.white(),
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// Castling rights
pub struct Sides<T = bool> {
    pub queen: T,
    pub king: T,
}

impl<T> ops::Index<Side> for Sides<T> {
    type Output = T;
    fn index(&self, side: Side) -> &T {
        self.get(side)
    }
}

impl<T> ops::IndexMut<Side> for Sides<T> {
    #[inline]
    fn index_mut(&mut self, side: Side) -> &mut T {
        self.get_mut(side)
    }
}

impl<T> Sides<T> {
    #[inline]
    pub const fn get(&self, side: Side) -> &T {
        match side {
            Side::Queen => &self.queen,
            Side::King => &self.king,
        }
    }

    #[inline]
    pub const fn get_mut(&mut self, side: Side) -> &mut T {
        match side {
            Side::Queen => &mut self.queen,
            Side::King => &mut self.king,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// Chess position
///
/// Size:
/// - board: 9 * 8 = 72 bytes
/// - turn: 1 byte (really 1 bit)
/// - castle rights: 2 bytes (really 2 bits)
/// - en passant square: 1 bytes (really 65 options)
/// - reversible move counter: 4 bytes (modeled as u32, really 1 byte would be enough)
/// - round counter: 4 bytes (value >=1, u16 or even u8 should be enough, who has 65536 rounds in a game of chess?)
///
/// Unpacked, this amounts to 88 bytes (Rust 1.91).
/// Packed it is 84 bytes.
///
/// <https://lichess.org/@/revoof/blog/adapting-nnue-pytorchs-binary-position-format-for-lichess/cpeeAMeY>
/// shows that about 18.7 bytes is enough
///
/// Besides size, a goal is also to stick these into SQLite3 (or DuckDB), and make something
/// similar to [Chess Query Language][cql] (with a less weird syntax...) efficently implementable.
///
/// The contents can be split in:
/// - board
/// - rights: turn + castle + en passant
/// - counters
///
/// [cql]: https://en.wikipedia.org/wiki/Chess_Query_Language
// #[repr(packed)]
pub struct Position<Variant = variant::Chess> {
    /// location of the pieces on the board
    pub board: Board,
    /// player to move
    pub turn: Player,
    /// possible castle sides
    pub castle: Players<Sides>,
    /// possible en passant square
    pub en_passant: Option<en_passant::Square>,
    /// ply counter since last capture or pawn move (reversible moves)
    pub reversible: u32,
    /// starts at 1 and increments after every Black move
    pub round: NonZeroU32,

    pub(crate) variant: Variant, //PhantomData<Variant>,
}

// initial: rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
// compact: rnbqkbnrpppppppp8888PPPPPPPPRNBQKBNRwKQkq
// compact:
// FEN: 6k1/7p/6r1/8/8/8/7P/5BRK b - - 0 1
// Compact FEN: 6k17p6r18887P5BRKb
// FEN: 6k1/7p/6r1/8/8/8/7P/5BRK b Kq e3 17 31
// Compact FEN: 6k17p6r18887P5BRK b Kq e3 17 31
//              6k17p6r18887P5BRK b 17 Kq 31 E
//              6k17p6r18887P5BRKb31Kq17E
// - intersperse 2 counters between 3 rights, swap counters (so b31 means 31..?)
// - drop default values (0 for ply, 1 for fullmove)
// - use e for e3 (white side) and E for e6 (black side)
// - drop defaults
//
// Characters:
// - board: {1..8}{p,n,b,q,k}{P,N,B,Q,K}
// - turn: {b,w}
//
// pub struct Position2 {
//     // What can we see?
//     pub board: Board,
//     // What can happen next?
//     pub right: Rights,
//     // Snippet of history
//     pub counter: Counters,
// }

// pub struct Counters {
//     pub reversible: u32,
//     pub round: NonZeroU32,
// }

// pub struct Rights{
//     pub turn: Player,
//     pub castle: Players<Sides>,
//     pub en_passant: Option<en_passant::Square>,
// }

// #[test]
// fn position_size() {
//     use core::mem::size_of as size;
//     panic!("castle: {}, board: {}, position: {}", size::<Players<Sides>>(), size::<Board>(), size::<Position>());
// }

pub type Unvalidated = Position<variant::Unvalidated>;

pub const fn unvalidated(board: Board, turn: Player) -> Unvalidated {
    Unvalidated {
        board,
        turn,
        castle: Players {
            black: Sides { queen: false, king: false },
            white: Sides { queen: false, king: false },
        },
        en_passant: None,
        reversible: 0,
        round: NonZeroU32::MIN,
        variant: variant::Unvalidated,
    }
}

impl<V: Variant> Position<V> {
    pub fn new(position: Unvalidated) -> Result<Self> {
        V::validate(position)
    }

    pub fn moves(&self) -> Moves {
        V::moves(self)
    }

    pub fn capture_moves(&self) -> Moves {
        self.moves().into_iter().filter(|m| m.is_capture()).collect()
    }

    pub fn castle_side_moves(&self, side: Side) -> Moves {
        self.moves().into_iter().filter(|m| m.is_castle_side(side)).collect()
    }

    pub fn castle_moves(&self) -> Moves {
        self.moves().into_iter().filter(|m| m.is_castle()).collect()
    }
}

// pub fn initial() -> Position {
//     let unvalidated = unvalidated();
//     Position { board: INITIAL, turn: Player::White, en_passant: None, ply_since: 0,
// }

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// Container (both players)
pub struct Players<T> {
    pub black: T,
    pub white: T,
}

impl<T> ops::Index<Player> for Players<T> {
    type Output = T;
    fn index(&self, player: Player) -> &T {
        self.get(player)
    }
}

impl<T> ops::IndexMut<Player> for Players<T> {
    #[inline]
    fn index_mut(&mut self, player: Player) -> &mut T {
        self.get_mut(player)
    }
}

impl<T> Players<T> {
    #[inline]
    pub const fn get(&self, player: Player) -> &T {
        match player {
            Player::Black => &self.black,
            Player::White => &self.white,
        }
    }

    #[inline]
    pub const fn get_mut(&mut self, player: Player) -> &mut T {
        match player {
            Player::Black => &mut self.black,
            Player::White => &mut self.white,
        }
    }

    #[inline]
    pub fn swap(self) -> Players<T> {
        Players { black: self.white, white: self.black }
    }

    #[inline]
    pub fn for_each<F>(self, mut f: F)
    where
        F: FnMut(T),
    {
        f(self.black);
        f(self.white);
    }

    #[inline]
    pub fn map<U, F>(self, mut f: F) -> Players<U>
    where
        F: FnMut(T) -> U,
    {
        Players { black: f(self.black), white: f(self.white) }
    }

    #[inline]
    pub fn find<F>(&self, mut predicate: F) -> Option<Player>
    where
        F: FnMut(&T) -> bool,
    {
        if predicate(&self.black) {
            Some(Player::Black)
        } else if predicate(&self.white) {
            Some(Player::White)
        } else {
            None
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// Container (all roles)
pub struct Roles<T> {
    pub pawn: T,
    pub knight: T,
    pub bishop: T,
    pub rook: T,
    pub queen: T,
    pub king: T,
}

impl<T> ops::Index<Role> for Roles<T> {
    type Output = T;
    fn index(&self, role: Role) -> &T {
        match role {
            Role::Pawn => &self.pawn,
            Role::Knight => &self.knight,
            Role::Bishop => &self.bishop,
            Role::Rook => &self.rook,
            Role::Queen => &self.queen,
            Role::King => &self.king,
        }
    }
}

impl<T> ops::IndexMut<Role> for Roles<T> {
    fn index_mut(&mut self, role: Role) -> &mut T {
        match role {
            Role::Pawn => &mut self.pawn,
            Role::Knight => &mut self.knight,
            Role::Bishop => &mut self.bishop,
            Role::Rook => &mut self.rook,
            Role::Queen => &mut self.queen,
            Role::King => &mut self.king,
        }
    }
}

impl<T> Roles<T> {
    #[inline]
    pub const fn get(&self, role: Role) -> &T {
        use Role::*;
        match role {
            Pawn => &self.pawn,
            Knight => &self.knight,
            Bishop => &self.bishop,
            Rook => &self.rook,
            Queen => &self.queen,
            King => &self.king,
        }
    }

    #[inline]
    pub const fn get_mut(&mut self, role: Role) -> &mut T {
        use Role::*;
        match role {
            Pawn => &mut self.pawn,
            Knight => &mut self.knight,
            Bishop => &mut self.bishop,
            Rook => &mut self.rook,
            Queen => &mut self.queen,
            King => &mut self.king,
        }
    }

    #[inline]
    pub fn for_each<F>(self, mut f: F)
    where
        F: FnMut(T),
    {
        f(self.pawn);
        f(self.knight);
        f(self.bishop);
        f(self.rook);
        f(self.queen);
        f(self.king);
    }

    #[inline]
    pub fn map<U, F>(self, mut f: F) -> Roles<U>
    where
        F: FnMut(T) -> U,
    {
        Roles {
            pawn: f(self.pawn),
            knight: f(self.knight),
            bishop: f(self.bishop),
            rook: f(self.rook),
            queen: f(self.queen),
            king: f(self.king),
        }
    }

    #[inline]
    pub fn find<F>(&self, mut predicate: F) -> Option<Role>
    where
        F: FnMut(&T) -> bool,
    {
        if predicate(&self.pawn) {
            Some(Role::Pawn)
        } else if predicate(&self.knight) {
            Some(Role::Knight)
        } else if predicate(&self.bishop) {
            Some(Role::Bishop)
        } else if predicate(&self.rook) {
            Some(Role::Rook)
        } else if predicate(&self.queen) {
            Some(Role::Queen)
        } else if predicate(&self.king) {
            Some(Role::King)
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn find_or_king<F>(&self, mut predicate: F) -> Role
    where
        F: FnMut(&T) -> bool,
    {
        if predicate(&self.pawn) {
            Role::Pawn
        } else if predicate(&self.knight) {
            Role::Knight
        } else if predicate(&self.bishop) {
            Role::Bishop
        } else if predicate(&self.rook) {
            Role::Rook
        } else if predicate(&self.queen) {
            Role::Queen
        } else {
            Role::King
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Special {
    EnPassant,
    Castle(Side),
    Promote(Role),
}

impl Special {
    #[inline]
    pub const fn en_passant() -> Self {
        EnPassant
    }

    #[inline]
    pub const fn castle(side: Side) -> Self {
        Castle(side)
    }

    #[inline]
    pub const fn promote(role: Role) -> Self {
        Promote(role)
    }
    #[inline]
    pub const fn is_en_passant(self) -> bool {
        matches!(self, Special::EnPassant)
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        matches!(self, Special::Castle(_))
    }

    #[inline]
    pub const fn castles(self) -> Option<Side> {
        if let Special::Castle(side) = self { Some(side) } else { None }
    }

    #[inline]
    pub const fn is_castle_side(self, side: Side) -> bool {
        if let Special::Castle(this) = self { this as u8 == side as u8 } else { false }
    }

    #[inline]
    pub const fn is_promote(self) -> bool {
        matches!(self, Special::Promote(_))
    }

    #[inline]
    pub const fn promotes(self) -> Option<Role> {
        if let Special::Promote(role) = self { Some(role) } else { None }
    }

    #[inline]
    pub const fn is_promote_role(self, role: Role) -> bool {
        if let Special::Promote(this) = self { this as u8 == role as u8 } else { false }
    }
}

/// What kind of move is it?
#[repr(u8)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Kind {
    #[default]
    Normal,
    Special(Special),
}

impl Kind {
    #[inline]
    pub const fn normal() -> Self {
        Normal
    }

    #[inline]
    pub const fn special(special: Special) -> Self {
        Kind::Special(special)
    }

    #[inline]
    pub const fn en_passant() -> Self {
        Kind::Special(EnPassant)
    }

    #[inline]
    pub const fn castle(side: Side) -> Self {
        Kind::Special(Castle(side))
    }

    #[inline]
    pub const fn promote(role: Role) -> Self {
        Kind::Special(Promote(role))
    }

    #[inline]
    pub const fn is_normal(self) -> bool {
        matches!(self, Kind::Normal)
    }

    #[inline]
    pub const fn is_special(self) -> bool {
        matches!(self, Kind::Normal)
    }

    #[inline]
    pub const fn specials(self) -> Option<Special> {
        if let Kind::Special(special) = self { Some(special) } else { None }
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        if let Kind::Special(special) = self { special.is_en_passant() } else { false }
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        if let Kind::Special(special) = self { special.is_castle() } else { false }
    }

    #[inline]
    pub const fn castles(self) -> Option<Side> {
        if let Kind::Special(special) = self { special.castles() } else { None }
    }

    #[inline]
    pub const fn is_castle_side(self, side: Side) -> bool {
        if let Kind::Special(special) = self { special.is_castle_side(side) } else { false }
    }

    #[inline]
    pub const fn is_promote(self) -> bool {
        if let Kind::Special(special) = self { special.is_promote() } else { false }
    }

    #[inline]
    pub const fn promotes(self) -> Option<Role> {
        if let Kind::Special(special) = self { special.promotes() } else { None }
    }

    #[inline]
    pub const fn is_promote_role(self, role: Role) -> bool {
        if let Kind::Special(special) = self { special.is_promote_role(role) } else { false }
    }
}

// Since `move` is a keyword, can use `play: Move` as a synonym.
// At least this is the most useful suggestion by Google AI Overview ;)
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
/// Move by a player
pub struct Move {
    pub kind: Kind,
    /// redundant when given the board
    pub role: Role,
    pub from: Square,
    pub to: Square,
    /// redundant when given the board
    pub capture: Option<Role>,
}

pub type Moves = Vec<Move>;

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
#[repr(u8)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(SerializeDisplay))]
pub enum Side {
    King = 0,
    Queen = 1,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::King => write!(f, "king"),
            Side::Queen => write!(f, "queen"),
        }
    }
}

impl Side {
    pub const ALL: [Side; 2] = [Side::King, Side::Queen];

    pub const fn king_side(king_side: bool) -> Self {
        use Side::*;
        if king_side { King } else { Queen }
    }

    /// The file the king moves to when castling on this side
    pub const fn king_to_file(self) -> File {
        use Side::*;
        match self {
            King => File::G,
            Queen => File::C,
        }
    }
}

impl All<2> for Side {
    const ALL: [Side; 2] = Side::ALL;
    fn index(self) -> usize {
        match self {
            Side::King => 0,
            Side::Queen => 1,
        }
    }
}

// Construct a normal move, or use the special constructors castle, en_passant, or promote.
//
// If a piece is captured, optionally set its role.

/// Constructors
impl Move {
    #[inline]
    pub const fn normal(role: Role, from: Square, to: Square) -> Move {
        Move { kind: Normal, role, from, to, capture: None }
    }

    #[inline]
    pub const fn castle(player: Player, side: Side) -> Move {
        let rank = player.backrank();
        Move {
            kind: Kind::Special(Castle(side)),
            role: Role::King,
            from: Square::new(File::E, rank),
            to: Square::new(side.king_to_file(), rank),
            capture: None,
        }
    }

    #[inline]
    pub const fn en_passant(from: Square, to: Square) -> Move {
        Move { role: Role::Pawn, from, to, capture: None, kind: Kind::Special(EnPassant) }
    }

    #[inline]
    pub const fn promote(from: Square, to: Square, role: Role) -> Move {
        Move { role: Role::Pawn, from, to, capture: None, kind: Kind::Special(Promote(role)) }
    }
}

/// Builders
impl Move {
    #[inline]
    pub const fn capturing(mut self, role: Role) -> Move {
        self.capture = Some(role);
        self
    }
}

/// Accessors inherited from [`Kind`]
impl Move {
    #[inline]
    pub const fn is_normal(self) -> bool {
        self.kind.is_normal()
    }

    #[inline]
    pub const fn is_special(self) -> bool {
        self.kind.is_special()
    }

    #[inline]
    pub const fn specials(self) -> Option<Special> {
        self.kind.specials()
    }
}

/// Accessors inherited from [`Special`]
impl Move {
    #[inline]
    pub const fn is_en_passant(self) -> bool {
        self.kind.is_en_passant()
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        self.kind.is_castle()
    }

    #[inline]
    pub const fn castles(self) -> Option<Side> {
        self.kind.castles()
    }

    #[inline]
    pub const fn is_castle_side(self, side: Side) -> bool {
        self.kind.is_castle_side(side)
    }

    #[inline]
    pub const fn is_promote(self) -> bool {
        self.kind.is_promote()
    }

    #[inline]
    pub const fn promotes(self) -> Option<Role> {
        self.kind.promotes()
    }

    #[inline]
    pub const fn is_promote_role(self, role: Role) -> bool {
        self.kind.is_promote_role(role)
    }
}

/// Accessors inherited from `capture`
impl Move {
    #[inline]
    pub const fn is_capture(self) -> bool {
        self.capture.is_some()
    }

    #[inline]
    pub const fn captures(self) -> Option<Role> {
        self.capture
    }

    #[inline]
    pub const fn is_capture_role(self, role: Role) -> bool {
        if let Some(this) = self.capture { this as u8 == role as u8 } else { false }
    }

    fn long_algebraic(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        let Move { role, from, to, capture, kind } = *self;

        if kind.is_castle() {
            f.write_str(if from < to { ALGEBRAIC_SHORT_CASTLE } else { ALGEBRAIC_LONG_CASTLE })
        } else {
            if role != Role::Pawn {
                f.write_char(role.upper())?;
            }
            let does = if capture.is_some() { ALGEBRAIC_CAPTURE } else { ALGEBRAIC_MOVE };
            write!(f, "{}{}{}", from, does, to)?;

            if let Some(promoted) = kind.promotes() {
                write!(f, "={}", promoted.upper())?;
            }
            Ok(())
        }
    }

    // In standard chess, castling is respresented as a move of the king to its new position
    // In freestyle, it would be a move to the corresponding rook square
    fn universal_chess_interface(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        write!(f, "{}{}", self.from, self.to)?;
        if let Some(promoted) = self.promotes() {
            f.write_char(promoted.lower())?;
        }
        Ok(())
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.long_algebraic(f)
    }
}

pub struct Uci(pub Move);

impl fmt::Display for Uci {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.universal_chess_interface(f)
    }
}

#[test]
fn display_move() {
    let mut play = Move::promote(Square::A2, Square::H7, Role::Queen);

    assert_eq!(play.to_string(), "a2-h7=Q");
    assert_eq!(Uci(play).to_string(), "a2h7q");

    play.role = Role::King;
    assert_eq!(play.to_string(), "Ka2-h7=Q");
    assert_eq!(Uci(play).to_string(), "a2h7q");

    play = play.capturing(Role::Bishop);
    assert_eq!(play.to_string(), "Ka2xh7=Q");
    assert_eq!(Uci(play).to_string(), "a2h7q");

    let play = Move::castle(Player::White, Side::King);
    assert_eq!(play.to_string(), ALGEBRAIC_SHORT_CASTLE);
    assert_eq!(Uci(play).to_string(), "e1g1");

    let play = Move::castle(Player::Black, Side::Queen);
    assert_eq!(play.to_string(), ALGEBRAIC_LONG_CASTLE);
    assert_eq!(Uci(play).to_string(), "e8c8");
}

pub struct Meta;

pub struct Game<V = variant::Chess> {
    pub meta: Meta,
    pub initial: Position<V>,
    pub current: Position<V>,
    pub moves: Moves,
}

pub struct Analysis<V = variant::Chess> {
    pub meta: Meta,
    pub initial: Position<V>,
    pub current: Position<V>,
    pub moves: Vec<MoveWithVariations>,
}

pub struct MoveWithVariations {
    pub play: Move,
    pub variations: Vec<MoveWithVariations>,
}

// pub struct Moves = Vec<Move>;

// pub struct Game<V = variant::Chess> {
//     pub meta: Meta,
//     pub initial: Position<V>,
//     pub current: Position<V>,
//     pub moves: Vec<Move>,
// }

#[test]
fn all_random() {
    use std::collections::BTreeSet as Set;

    let mut positions = Vec::new();
    let all = Set::from_iter(1usize..=8);
    for rook_l in 1usize..=8 {
        for rook_r in rook_l + 2..=8 {
            for king in (rook_l + 1)..rook_r {
                assert!(rook_l < king);
                assert!(king < rook_r);
                let it = (1usize..rook_l)
                    .chain((rook_l + 1)..king)
                    .chain((king + 1)..rook_r)
                    .chain((rook_r + 1)..=8);
                for bishop_b in it.clone().filter(|i| (i & 1) == 1) {
                    for bishop_w in it.clone().filter(|i| (i & 1) == 0) {
                        let used = Set::from([rook_l, rook_r, king, bishop_b, bishop_w]);
                        let remaining = all.difference(&used);
                        for queen in remaining.clone() {
                            let mut position = vec!['-'; 8];
                            position[rook_l - 1] = 'r';
                            position[rook_r - 1] = 'r';
                            position[king - 1] = 'k';
                            position[queen - 1] = 'q';
                            position[bishop_w - 1] = 'b';
                            position[bishop_b - 1] = 'b';
                            for knight in remaining.clone() {
                                if knight != queen {
                                    position[knight - 1] = 'n';
                                }
                            }
                            assert!(!position.contains(&'-'));
                            positions.push(position);
                        }
                    }
                }
            }
        }
    }
    for position in positions.iter() {
        println!("{}", position.iter().collect::<String>());
    }
    assert_eq!(960, positions.len());
}

pub trait All<const N: usize>: Copy + Sized {
    const ALL: [Self; N];

    fn index(self) -> usize;

    fn all() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter()
    }
}

impl All<2> for Player {
    const ALL: [Player; 2] = Player::ALL;
    fn index(self) -> usize {
        match self {
            Player::Black => 0,
            Player::White => 1,
        }
    }
}

/// "Full" map, containing an element of T for any element of X
///
/// They are accessed by indexing, e.g. `let t = map[x]` and `map[x] = t`
///
/// Note that `[T; N]` does not always implement Default, e.g. for `N = 64`.
/// For this reason, we define ad-hoc inherent methods `fn default64()` for
/// all `T: Default` on all `Map<T, X, 64>` etc. Note that we can implement
/// functions named `fn default()`, but the compiler will get confused by
/// use of `Map::default()` for those N where usual Default *is* defined...
#[derive(Clone, Copy, Debug)]
pub struct Map<X, T, const N: usize> {
    all: [T; N],
    __: PhantomData<X>,
}

impl<X, T, const N: usize> From<Map<X, T, N>> for BTreeMap<X, T>
where
    T: Copy,
    X: All<N> + Ord,
{
    fn from(map: Map<X, T, N>) -> Self {
        let mut bmap = BTreeMap::new();
        for x in 0..N {
            bmap.insert(X::ALL[x], map.all[x]);
        }
        bmap
    }
}

// #[cfg(feature = "serde")]
// impl<'de, X, T: Deserialize<'de>, const N: usize> Deserialize<'de> for Map<X, T, N> {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         let all: Vec<T> = Deserialize::deserialize(deserializer)?;
//         let all: [T; N] = all.try_into().map_err(|_| serde::de::Error::custom("wrong length"))?;
//         Ok(Self { all, __: PhantomData })
//     }
// }

#[cfg(feature = "serde")]
impl<X, T: Serialize, const N: usize> Serialize for Map<X, T, N>
where
    T: Copy + Serialize,
    X: All<N> + Ord + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Serialize::serialize(&BTreeMap::from(*self), serializer)
    }
}

impl<X, T: Default, const N: usize> Default for Map<X, T, N>
where
    [T; N]: Default,
{
    fn default() -> Self {
        Self { all: Default::default(), __: PhantomData }
    }
}

impl<X, T: Copy + Default> Map<X, T, 64> {
    /// Need this because [T; 64] does not implement Default for historical reasons
    fn default64() -> Self {
        Self { all: [T::default(); 64], __: PhantomData }
    }
}

impl<X: All<N>, T, const N: usize> ops::Index<X> for Map<X, T, N> {
    type Output = T;
    fn index(&self, value: X) -> &T {
        &self.all[value.index()]
    }
}

impl<X: All<N>, T, const N: usize> ops::IndexMut<X> for Map<X, T, N> {
    fn index_mut(&mut self, value: X) -> &mut T {
        &mut self.all[value.index()]
    }
}
