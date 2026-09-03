use std::str::FromStr;

use crate::{bitboard::Bitboard, finite::Empty as _};

/// A chess piece, for instance a white pawn or a black queen.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Piece {
    pub player: Player,
    pub role: Role,
}

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

crate::finite_set!(
    /// A supported chess variant.
    #[non_exhaustive]
    VariantEnum,
    VariantTable {
        Unvalidated = 0 as unvalidated,
        Chess = 1 as chess,
        Freestyle = 2 as freestyle,
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

    #[inline]
    pub const fn upper(self) -> char {
        use File::*;
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

impl Piece {
    #[inline]
    pub const fn from_char(c: char) -> Option<Self> {
        let player = if c.is_ascii_lowercase() { Player::Black } else { Player::White };
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
            Player::Black => self.role.black(),
            Player::White => self.role.white(),
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
        matches!(self, Player::Black)
    }

    #[inline]
    pub const fn is_white(self) -> bool {
        matches!(self, Player::White)
    }

    #[inline]
    pub const fn other(self) -> Player {
        match self {
            Player::Black => Player::White,
            Player::White => Player::Black,
        }
    }

    #[inline]
    pub const fn backrank(self) -> Rank {
        match self {
            Player::Black => Rank::Eight,
            Player::White => Rank::One,
        }
    }

    #[inline]
    pub const fn pawn_start_rank(self) -> Rank {
        match self {
            Player::Black => Rank::Seven,
            Player::White => Rank::Two,
        }
    }

    #[inline]
    pub const fn promotion_rank(self) -> Rank {
        match self {
            Player::Black => Rank::One,
            Player::White => Rank::Eight,
        }
    }

    /// `White.pawn()`. `Role::of` is the inverse spelling for `Pawn.of(White)`.
    #[inline]
    pub const fn pawn(self) -> Piece {
        Role::Pawn.of(self)
    }

    #[inline]
    pub const fn knight(self) -> Piece {
        Role::Knight.of(self)
    }

    #[inline]
    pub const fn bishop(self) -> Piece {
        Role::Bishop.of(self)
    }

    #[inline]
    pub const fn rook(self) -> Piece {
        Role::Rook.of(self)
    }

    #[inline]
    pub const fn queen(self) -> Piece {
        Role::Queen.of(self)
    }

    #[inline]
    pub const fn king(self) -> Piece {
        Role::King.of(self)
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
            .union_const(rook_path)
            .difference_const(Bitboard::from_square(king_from))
            .difference_const(Bitboard::from_square(rook_from))
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

impl Role {
    #[inline]
    pub const fn from_char(c: char) -> Option<Self> {
        use Role::*;
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

    #[inline]
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

    #[inline]
    pub const fn figurine(self) -> char {
        use Role::*;
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

impl<T> Roles<T> {
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
}

impl Side {
    #[inline]
    pub const fn chess_rook(self) -> File {
        match self {
            Side::King => File::H,
            Side::Queen => File::A,
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
            King => File::G,
            Queen => File::C,
        }
    }

    /// The file the rook moves to when castling on this side.
    #[inline]
    pub const fn rook_to_file(self) -> File {
        use Side::*;
        match self {
            King => File::F,
            Queen => File::D,
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
