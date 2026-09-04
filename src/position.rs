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
#[cfg(test)]
extern crate alloc;

use core::{marker::PhantomData, num::NonZeroU32};

use crate::{Board, Piece, Player, Role, Scharnagl, Square, board::Bitboard, variant};

use Player::*;
use Role::*;

pub use variant::{Chess, Freestyle, SupportedEnum, Unvalidated, Variant, VariantEnum};

mod moves;
mod play;
mod rights;
mod validate;

pub use play::*;
pub use rights::*;

pub use Kind::Normal;
pub use Special::{Castle, Promote};

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
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Position<Variant = Chess> {
    /// location of the pieces on the board
    board: Board,
    /// player to move
    turn: Player,
    /// possible castle sides
    castles: Castles,
    /// possible en passant square
    en_passant: Option<EnPassant>,
    /// ply counter since last capture or pawn move (reversible moves)
    reversible: u32,
    /// starts at 1 and increments after every Black move
    round: NonZeroU32,

    pub(crate) variant: PhantomData<Variant>,
}

// Here and elsewhere, a #[derive(Clone, Copy)] won't work due to
// derive macro limitations - Position is in fact Copy
impl<V> Copy for Position<V> {}

impl<V> Clone for Position<V> {
    fn clone(&self) -> Self {
        *self
    }
}
/// Equivalent to [`Position<Unvalidated>`].
///
/// We want to keep `V: Validate` position fields private,
/// so we can uphold the validated invariants.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Parts {
    /// location of the pieces on the board
    pub board: Board,
    /// player to move
    pub turn: Player,
    /// possible castle sides
    pub castles: Castles,
    /// possible en passant square
    pub en_passant: Option<EnPassant>,
    /// ply counter since last capture or pawn move (reversible moves)
    pub reversible: u32,
    /// starts at 1 and increments after every Black move
    pub round: NonZeroU32,
}

impl Parts {
    pub const fn position(self) -> Position<Unvalidated> {
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
}

impl From<Parts> for Position<Unvalidated> {
    fn from(parts: Parts) -> Self {
        parts.position()
    }
}

pub const fn unvalidated(board: Board, turn: Player) -> Position<Unvalidated> {
    Position {
        board,
        turn,
        castles: Castles::empty(),
        en_passant: None,
        reversible: 0,
        round: NonZeroU32::MIN,
        variant: PhantomData,
    }
}

impl Position<Chess> {
    pub const fn start() -> Position<Chess> {
        Position {
            board: Board::standard(),
            turn: White,
            castles: Castles::chess(),
            en_passant: None,
            reversible: 0,
            round: NonZeroU32::MIN,
            variant: PhantomData,
        }
    }
}

impl Position<Freestyle> {
    pub const fn freestyle(i: Scharnagl) -> Position<Freestyle> {
        // Construct board
        let board = Board::freestyle(i);

        // Extract castle rights
        let king = board.king_of(White).unwrap();
        let mut rooks = board.rooks();
        let queen_rook = rooks.pop_first().unwrap().file();
        let king_rook = rooks.first().unwrap().file();

        let mut castles = Castles::empty();
        castles.set(White, Side::of_rook(king, queen_rook), queen_rook);
        castles.set(White, Side::of_rook(king, king_rook), king_rook);
        castles.set(Black, Side::Queen, queen_rook);
        castles.set(Black, Side::King, king_rook);

        Position {
            board,
            turn: White,
            castles,
            en_passant: None,
            reversible: 0,
            round: NonZeroU32::MIN,
            variant: PhantomData,
        }
    }
}

impl Position<Unvalidated> {
    pub const fn chess() -> Position<Unvalidated> {
        Position {
            board: Board::standard(),
            turn: White,
            castles: Castles::chess(),
            en_passant: None,
            reversible: 0,
            round: NonZeroU32::MIN,
            variant: PhantomData,
        }
    }

    pub const fn empty() -> Position<Unvalidated> {
        unvalidated(Board::EMPTY, White)
    }

    pub fn set_piece(&mut self, square: Square, piece: Piece) -> Option<Piece> {
        self.board.insert(square, piece)
    }

    pub fn remove_piece(&mut self, square: Square) -> Option<Piece> {
        self.board.remove(square)
    }

    pub fn move_piece(&mut self, from: Square, to: Square) -> Result<Option<Piece>> {
        let Some(piece) = self.board.remove(from) else {
            return Err(Error::MissingPiece(from));
        };
        let captured = self.board.insert(to, piece);
        Ok(captured)
    }
}

impl Default for Position<Unvalidated> {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<Position<Chess>> for Position<Unvalidated> {
    fn from(position: Position<Chess>) -> Self {
        position.unvalidated()
    }
}

impl From<Position<Freestyle>> for Position<Unvalidated> {
    fn from(position: Position<Freestyle>) -> Self {
        position.unvalidated()
    }
}

// impl<V: Validate> Position<V> {
//     pub fn new(position: Unvalidated) -> Result<Self> {
//         position.validate()
//     }
// }

impl<V> Position<V> {
    #[inline]
    pub const fn board(&self) -> Board {
        self.board
    }

    #[inline]
    pub const fn turn(&self) -> Player {
        self.turn
    }

    #[inline]
    pub const fn castles(&self) -> Castles {
        self.castles
    }

    #[inline]
    pub const fn en_passant(&self) -> Option<EnPassant> {
        self.en_passant
    }

    #[inline]
    pub const fn reversible(&self) -> u32 {
        self.reversible
    }

    #[inline]
    pub const fn round(&self) -> NonZeroU32 {
        self.round
    }

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

impl<V: Variant> Position<V> {
    pub fn capture_moves(&self) -> Vec<Move> {
        self.legal_moves().into_iter().filter(|m| m.is_capture()).collect()
    }

    pub fn castle_side_moves(&self, side: Side) -> Vec<Move> {
        self.legal_moves().into_iter().filter(|m| m.is_castle_side(side)).collect()
    }

    pub fn castle_moves(&self) -> Vec<Move> {
        self.legal_moves().into_iter().filter(|m| m.is_castle()).collect()
    }
}

impl<V> Position<V> {
    pub const fn parts(self) -> Parts {
        Parts {
            board: self.board,
            turn: self.turn,
            castles: self.castles,
            en_passant: self.en_passant,
            reversible: self.reversible,
            round: self.round,
        }
    }

    pub fn first_ply(&self) -> usize {
        let round = self.round.get() as usize - 1;
        round * 2 + usize::from(self.turn == Black)
    }

    pub const fn unvalidated(self) -> Position<Unvalidated> {
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
}

impl<V> From<Position<V>> for Parts {
    fn from(position: Position<V>) -> Self {
        position.parts()
    }
}

impl<V: Variant> Position<V> {
    pub(crate) fn apply_unchecked(mut self, play: Move) -> Position<V> {
        let player = self.turn;
        let captured = if play.is_en_passant() {
            Some(Square::new(play.to.file(), play.from.rank()))
        } else if play.capture.is_some() {
            Some(play.to)
        } else {
            None
        };

        if play.role == King {
            self.castles.clear_player(player);
        }

        if play.role == Rook {
            self.clear_castle_rook(player, play.from);
        }

        if let Some(captured) = captured {
            self.clear_castle_rook(player.other(), captured);
        }

        self.board.play_unchecked(player, play);

        let mut en_passant = None;
        if play.role == Pawn {
            let from = play.from as u8;
            let to = play.to as u8;
            if from.abs_diff(to) == 16 {
                en_passant = EnPassant::try_from(Square::panicky_from_index((from + to) / 2)).ok();
            }
        }

        if play.role == Pawn || play.capture.is_some() || play.is_en_passant() {
            self.reversible = 0;
        } else {
            self.reversible += 1;
        }

        if player == Black {
            self.round = self.round.saturating_add(1);
        }

        self.turn = player.other();
        self.en_passant = match en_passant {
            Some(en_passant) => self.attacked_en_passant(en_passant),
            None => None,
        };
        self
    }

    fn clear_castle_rook(&mut self, player: Player, square: Square) {
        if square.rank() != player.backrank() {
            return;
        }

        for side in Side::ALL {
            if self.castles.get(player, side) == Some(square.file()) {
                self.castles.clear(player, side);
            }
        }
    }
}

#[test]
fn all_random() {
    use alloc::collections::BTreeSet as Set;

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
