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
use core::{fmt, marker::PhantomData, num::NonZeroU32};

use crate::{
    bitboard::Bitboard,
    variant::{self, Variant},
};

pub mod en_passant;
pub mod square;
pub mod validate;

pub use square::{File, Rank, Square};

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

crate::finite_set!(
    /// Black and white
    Player,
    Players,
    PlayerTable,
    Players {
        Black = 0 as black,
        White = 1 as white,
    }
);

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Player::Black => write!(f, "black"),
            Player::White => write!(f, "white"),
        }
    }
}

impl Player {
    pub const fn eq(self, other: Player) -> bool {
        self as u8 == other as u8
    }

    pub const fn other(self) -> Player {
        match self {
            Player::Black => Player::White,
            Player::White => Player::Black,
        }
    }

    pub const fn backrank(self) -> Rank {
        match self {
            Player::Black => Rank::Eight,
            Player::White => Rank::One,
        }
    }

    pub const fn pawn_start_rank(self) -> Rank {
        match self {
            Player::Black => Rank::Seven,
            Player::White => Rank::Two,
        }
    }

    pub const fn promotion_rank(self) -> Rank {
        match self {
            Player::Black => Rank::One,
            Player::White => Rank::Eight,
        }
    }
}

// This has 1 + 2 players + 6 roles, so should be 9 * 8 = 72 bytes
//
// The `occupied` field is redundant, at which point it would be 64 bytes or 512 bits.
// Maybe this fits in AVX-512 registers?
//
// Invariant: players disjoint, roles disjoint, both union to same (=occupied)
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
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
        self.players.get(player)
    }

    #[inline]
    pub const fn role(self, role: Role) -> Bitboard {
        self.roles.get(role)
    }

    pub fn add(&mut self, square: Square, piece: Piece) {
        self.occupied.insert(square);
        self.players[piece.player].insert(square);
        self.roles[piece.role].insert(square);
    }

    pub fn remove(&mut self, square: Square) -> Option<Piece> {
        let piece = self.piece_at(square)?;
        self.occupied.remove(square);
        self.players[piece.player].remove(square);
        self.roles[piece.role].remove(square);
        Some(piece)
    }

    fn play_unchecked(&mut self, player: Player, play: Move) {
        let Some(special) = play.specials() else {
            self.remove(play.from);
            self.remove(play.to);
            self.add(play.to, play.role.of(player));
            return;
        };

        match special {
            EnPassant => {
                let captured = Square::new(play.to.file(), play.from.rank());
                self.remove(play.from);
                self.remove(captured);
                self.add(play.to, Role::Pawn.of(player));
            }
            Castle(side) => {
                let rook_from = standard_castle_rook(player, side);
                let rook_to = standard_castle_rook_to(player, side);
                self.remove(play.from);
                self.remove(rook_from);
                self.add(play.to, Role::King.of(player));
                self.add(rook_to, Role::Rook.of(player));
            }
            Promote(role) => {
                self.remove(play.from);
                self.remove(play.to);
                self.add(play.to, role.of(player));
            }
        }
    }

    /// Bishops, rooks and queens.
    #[inline]
    pub const fn sliders(self) -> Bitboard {
        let Roles { bishop, rook, queen, .. } = self.roles;
        bishop.symmetric_difference_const(rook).symmetric_difference_const(queen)
    }

    #[inline]
    pub const fn bishops_and_queens(self) -> Bitboard {
        self.bishops().union_const(self.queens())
    }

    #[inline]
    pub const fn rooks_and_queens(self) -> Bitboard {
        self.rooks().union_const(self.queens())
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
    pub const fn player_at(self, square: Square) -> Option<Player> {
        // not using Board::find to stay const fn
        if self.players.black.contains(square) {
            Some(Player::Black)
        } else if self.players.white.contains(square) {
            Some(Player::White)
        } else {
            None
        }
    }

    #[inline]
    pub const fn role_at(self, square: Square) -> Option<Role> {
        // not using Board::find to stay const fn
        if !self.occupied.contains(square) {
            // early return
            return None;
        }
        if self.roles.pawn.contains(square) {
            Some(Role::Pawn)
        } else if self.roles.knight.contains(square) {
            Some(Role::Knight)
        } else if self.roles.bishop.contains(square) {
            Some(Role::Bishop)
        } else if self.roles.rook.contains(square) {
            Some(Role::Rook)
        } else if self.roles.queen.contains(square) {
            Some(Role::Queen)
        } else if self.roles.king.contains(square) {
            Some(Role::King)
        } else {
            None
        }
    }

    #[inline]
    pub const fn piece_at(self, square: Square) -> Option<Piece> {
        match (self.player_at(square), self.role_at(square)) {
            (Some(player), Some(role)) => Some(role.of(player)),
            _ => None,
        }
    }
}

const fn standard_castle_rook(player: Player, side: Side) -> Square {
    let file = match side {
        Side::King => File::H,
        Side::Queen => File::A,
    };
    Square::new(file, player.backrank())
}

const fn standard_castle_rook_to(player: Player, side: Side) -> Square {
    let file = match side {
        Side::King => File::F,
        Side::Queen => File::D,
    };
    Square::new(file, player.backrank())
}

// pub const INITIAL: Board = Board;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("inconsistent board bitboards")]
    InconsistentBoard,
    #[error("expected exactly one {0} king")]
    KingCount(Player),
    #[error("pawns cannot be on the first or eighth rank")]
    PawnOnBackrank,
    #[error("kings cannot be adjacent")]
    AdjacentKings,
    #[error("{0} king is attacked")]
    KingAttacked(Player),
    #[error("invalid {0} {1:?}-side castling right")]
    Castling(Player, Side),
    #[error("no piece on {0}")]
    MissingPiece(Square),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

crate::finite_set!(
    /// A chess piece role, such as pawn, knight, bishop, etc.
    Role,
    Roles,
    RoleTable,
    Roles {
        Pawn = 1 as pawn,
        Knight = 2 as knight,
        Bishop = 3 as bishop,
        Rook = 5 as rook,
        Queen = 9 as queen,
        King = 4 as king,
    }
);

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

impl Role {
    pub const fn eq(self, other: Role) -> bool {
        self as u8 == other as u8
    }

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

    pub const fn black(self) -> char {
        self.lower()
    }

    pub const fn white(self) -> char {
        self.upper()
    }
}

/// A chess piece, for instance a white pawn or a black queen.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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

    // Eq::eq is not const
    pub const fn eq(self, other: Piece) -> bool {
        self.player.eq(other.player) && self.role.eq(other.role)
    }
}

/// Chess position, including the board, turn, rights, and counters.
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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Position<Variant = variant::Chess> {
    /// location of the pieces on the board
    pub board: Board,
    /// player to move
    pub turn: Player,
    /// possible castle sides
    pub castles: Castles,
    /// possible en passant square
    pub en_passant: Option<en_passant::Square>,
    /// ply counter since last capture or pawn move (reversible moves)
    pub reversible: u32,
    /// starts at 1 and increments after every Black move
    pub round: NonZeroU32,

    pub(crate) variant: PhantomData<Variant>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Castles(pub Players<Sides<Option<File>>>);

impl Side {
    pub const fn chess_rook(self) -> File {
        match self {
            Side::King => File::H,
            Side::Queen => File::A,
        }
    }
}

impl Square {
    pub const fn chess_rook(player: super::Player, side: super::Side) -> Self {
        Self::castle_rook(player, side.chess_rook())
    }

    pub const fn castle_rook(player: Player, file: File) -> Self {
        Self::new(file, player.backrank())
    }
}

impl<T> Sides<Option<T>> {
    pub const fn empty() -> Self {
        Sides { queen: None, king: None }
    }
}
impl Castles {
    pub const fn empty() -> Self {
        Self(Players { black: Sides::empty(), white: Sides::empty() })
    }

    pub const fn chess() -> Self {
        use Side::*;
        let rooks = Sides { queen: Some(Queen.chess_rook()), king: Some(King.chess_rook()) };
        Self(Players { black: rooks, white: rooks })
    }

    pub const fn get(self, player: Player, side: Side) -> Option<File> {
        self.0.get(player).get(side)
    }

    pub const fn has(self, player: Player, side: Side) -> bool {
        self.get(player, side).is_some()
    }

    pub fn set(&mut self, player: Player, side: Side, file: File) {
        self.0[player][side] = Some(file);
    }

    pub fn clear(&mut self, player: Player, side: Side) {
        self.0[player][side] = None;
    }

    pub fn clear_player(&mut self, player: Player) {
        self.0[player] = Sides::empty();
    }
}

pub type Unvalidated = Position<variant::Unvalidated>;

pub const fn unvalidated(board: Board, turn: Player) -> Unvalidated {
    Unvalidated {
        board,
        turn,
        castles: Castles::empty(),
        en_passant: None,
        reversible: 0,
        round: NonZeroU32::MIN,
        variant: PhantomData,
    }
}

impl Position<variant::Chess> {
    pub const fn start() -> Position<variant::Chess> {
        Position {
            board: Board::standard(),
            turn: Player::White,
            castles: Castles::chess(),
            en_passant: None,
            reversible: 0,
            round: NonZeroU32::MIN,
            variant: PhantomData,
        }
    }
}

impl Position<variant::Unvalidated> {
    pub const fn chess() -> Position<variant::Unvalidated> {
        Position {
            board: Board::standard(),
            turn: Player::White,
            castles: Castles::chess(),
            en_passant: None,
            reversible: 0,
            round: NonZeroU32::MIN,
            variant: PhantomData,
        }
    }

    pub const fn empty() -> Position<variant::Unvalidated> {
        unvalidated(Board::empty(), Player::White)
    }

    pub fn set_piece(&mut self, square: Square, piece: Piece) -> Option<Piece> {
        let previous = self.board.remove(square);
        self.board.add(square, piece);
        previous
    }

    pub fn remove_piece(&mut self, square: Square) -> Option<Piece> {
        self.board.remove(square)
    }

    pub fn move_piece(&mut self, from: Square, to: Square) -> Result<Option<Piece>> {
        let Some(piece) = self.board.remove(from) else {
            return Err(Error::MissingPiece(from));
        };
        let captured = self.board.remove(to);
        self.board.add(to, piece);
        Ok(captured)
    }
}

impl Default for Unvalidated {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<Position<variant::Chess>> for Unvalidated {
    fn from(position: Position<variant::Chess>) -> Self {
        position.unvalidated()
    }
}

// impl<V: Validate> Position<V> {
//     pub fn new(position: Unvalidated) -> Result<Self> {
//         position.validate()
//     }
// }

impl<V: Variant> Position<V> {
    pub const fn checkers(&self) -> Bitboard {
        match self.board.king_of(self.turn) {
            Some(king) => self.board.attacks_on(king, self.turn.other(), self.board.occupied()),
            None => Bitboard::EMPTY,
        }
    }

    pub const fn is_check(&self) -> bool {
        !self.checkers().is_empty()
    }
}

impl<V: variant::CanCastle> Position<V> {
    pub fn capture_moves(&self) -> Moves {
        self.legal_moves().into_iter().filter(|m| m.is_capture()).collect()
    }

    pub fn castle_side_moves(&self, side: Side) -> Moves {
        self.legal_moves().into_iter().filter(|m| m.is_castle_side(side)).collect()
    }

    pub fn castle_moves(&self) -> Moves {
        self.legal_moves().into_iter().filter(|m| m.is_castle()).collect()
    }
}

impl<V> Position<V> {
    pub fn first_ply(&self) -> usize {
        let round = self.round.get() as usize - 1;
        round * 2 + usize::from(self.turn == Player::Black)
    }

    pub fn unvalidated(self) -> Position<variant::Unvalidated> {
        Position {
            board: self.board,
            turn: self.turn,
            castles: self.castles,
            en_passant: self.en_passant,
            reversible: self.reversible,
            round: self.round,
            variant: PhantomData,
        }
    }

    pub(crate) fn apply_unchecked(mut self, play: Move) -> Position<V> {
        let player = self.turn;
        let captured = if play.is_en_passant() {
            Some(Square::new(play.to.file(), play.from.rank()))
        } else if play.capture.is_some() {
            Some(play.to)
        } else {
            None
        };

        if play.role == Role::King {
            self.castles.clear_player(player);
        }

        if play.role == Role::Rook {
            self.clear_standard_castle_rook(player, play.from);
        }

        if let Some(captured) = captured {
            self.clear_standard_castle_rook(player.other(), captured);
        }

        self.board.play_unchecked(player, play);

        self.en_passant = None;
        if play.role == Role::Pawn {
            let from = play.from as u8;
            let to = play.to as u8;
            if from.abs_diff(to) == 16 {
                self.en_passant =
                    en_passant::Square::try_from(Square::panicky_from_index((from + to) / 2)).ok();
            }
        }

        if play.role == Role::Pawn || play.capture.is_some() || play.is_en_passant() {
            self.reversible = 0;
        } else {
            self.reversible += 1;
        }

        if player == Player::Black {
            self.round = self.round.saturating_add(1);
        }

        self.turn = player.other();
        self
    }

    fn clear_standard_castle_rook(&mut self, player: Player, square: Square) {
        let rank = player.backrank();
        if square == Square::new(File::A, rank) {
            self.castles.clear(player, Side::Queen);
        } else if square == Square::new(File::H, rank) {
            self.castles.clear(player, Side::King);
        }
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

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Move that a player can make.
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
crate::finite_set!(
    /// A side of the board to castle toward.
    Side,
    Sides,
    SideTable,
    Sides {
        King = 0 as king,
        Queen = 1 as queen,
    }
);

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::King => write!(f, "king"),
            Side::Queen => write!(f, "queen"),
        }
    }
}

impl Side {
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

// Construct a normal move, or use the special constructors castle, en_passant, or promote.
//
// If a piece is captured, optionally set its role.

/// Constructors
impl Move {
    #[inline]
    pub const fn normal(role: Role, from: Square, to: Square) -> Move {
        Move::capture(role, from, to, None)
    }

    #[inline]
    pub const fn capture(role: Role, from: Square, to: Square, capture: Option<Role>) -> Move {
        Move { kind: Normal, role, from, to, capture }
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
        Move::promote_capture(from, to, role, None)
    }

    #[inline]
    pub const fn promote_capture(
        from: Square,
        to: Square,
        role: Role,
        capture: Option<Role>,
    ) -> Move {
        Move { role: Role::Pawn, from, to, capture, kind: Kind::Special(Promote(role)) }
    }

    pub fn pawn(player: Player, from: Square, to: Square, capture: Option<Role>) -> Moves {
        let mut moves = Moves::new();

        if to.rank() as u8 == player.promotion_rank() as u8 {
            for role in [Role::Queen, Role::Rook, Role::Bishop, Role::Knight] {
                moves.push(Move::promote_capture(from, to, role, capture));
            }
        } else {
            moves.push(Move::capture(Role::Pawn, from, to, capture));
        }

        moves
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
}

/// Compact encoding
impl Move {
    #[inline]
    pub const fn code(self) -> u16 {
        (self.to as u16)
            | ((self.from as u16) << 6)
            | (self.promotion_code() << 12)
            | (self.kind_code() << 14)
    }

    #[inline]
    const fn promotion_code(self) -> u16 {
        match self.promotes() {
            Some(Role::Knight) => 0,
            Some(Role::Bishop) => 1,
            Some(Role::Rook) => 2,
            Some(Role::Queen) => 3,
            _ => 0,
        }
    }

    #[inline]
    const fn kind_code(self) -> u16 {
        match self.specials() {
            Some(Special::Promote(_)) => 1,
            Some(Special::EnPassant) => 2,
            Some(Special::Castle(_)) => 3,
            None => 0,
        }
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
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.long_algebraic(f)
    }
}

#[test]
fn display_move() {
    let mut play = Move::promote(Square::A2, Square::H7, Role::Queen);

    assert_eq!(play.to_string(), "a2-h7=Q");
    assert_eq!(play.uci().to_string(), "a2h7q");

    play.role = Role::King;
    assert_eq!(play.to_string(), "Ka2-h7=Q");
    assert_eq!(play.uci().to_string(), "a2h7q");

    play = play.capturing(Role::Bishop);
    assert_eq!(play.to_string(), "Ka2xh7=Q");
    assert_eq!(play.uci().to_string(), "a2h7q");

    let play = Move::castle(Player::White, Side::King);
    assert_eq!(play.to_string(), ALGEBRAIC_SHORT_CASTLE);
    assert_eq!(play.uci().to_string(), "e1g1");

    let play = Move::castle(Player::Black, Side::Queen);
    assert_eq!(play.to_string(), ALGEBRAIC_LONG_CASTLE);
    assert_eq!(play.uci().to_string(), "e8c8");
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
