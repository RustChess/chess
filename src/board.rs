use core::{fmt, ops};

use crate::{
    Move, Side, Square, finite_for,
    position::{Error, Result, Special},
    square::{File, Rank},
};

mod bitboard;
mod moves;
mod piece;
mod scharnagl;

pub use bitboard::Bitboard;
pub use piece::*;
pub use scharnagl::{Scharnagl, scharnagl_by_id};

pub use Player::*;
pub use Role::*;

// This has 1 + 2 players + 6 roles, so should be 9 * 8 = 72 bytes
//
// The `occupied` field is redundant, at which point it would be 64 bytes or 512 bits.
// Maybe this fits in AVX-512 registers?
//
// Invariant: players disjoint, roles disjoint, both union to same (=occupied)
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
/// Validated location of pieces on the board.
///
/// A board is structurally valid when:
/// - the Black and White squares are disjoint;
/// - the six role bitboards are pairwise disjoint;
/// - the union of the player bitboards equals the union of the role bitboards; and
/// - `occupied` is that shared union.
///
/// This does not imply that the board is a legal chess position.
pub struct Board {
    occupied: Bitboard,
    players: Players<Bitboard>,
    roles: Roles<Bitboard>,
}

/// Unvalidated location of pieces on the board, without the redundant occupied cache.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Parts {
    pub players: Players<Bitboard>,
    pub roles: Roles<Bitboard>,
}

impl fmt::Debug for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use fmt::Write as _;

        for rank in Rank::ALL.into_iter().rev() {
            for file in File::ALL {
                let square = Square::new(file, rank);
                f.write_char(self.get(square).map_or('.', Piece::char))?;
                f.write_char(if file < File::H { ' ' } else { '\n' })?;
            }
        }

        Ok(())
    }
}

/// Board Construction API.
impl Board {
    pub const EMPTY: Board = Board {
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
    };

    #[inline]
    pub const fn freestyle(i: Scharnagl) -> Board {
        i.board()
    }

    #[inline]
    pub const fn new(parts: Parts) -> Result<Board> {
        let players = parts.players.black.union(parts.players.white);
        if !parts.players.black.is_disjoint(parts.players.white) {
            return Err(Error::InconsistentBoard);
        }

        let mut roles = Bitboard::EMPTY;
        finite_for!(role in Role {
            let squares = parts.roles.get(role);
            if !roles.is_disjoint(squares) {
                return Err(Error::InconsistentBoard);
            }
            roles.append(squares);
        });

        if !players.eq(roles) {
            return Err(Error::InconsistentBoard);
        }

        Ok(Board { occupied: players, players: parts.players, roles: parts.roles })
    }

    #[inline]
    pub const fn parts(self) -> Parts {
        Parts { players: self.players, roles: self.roles }
    }

    #[inline]
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
}

/// Board Chess API.
impl Board {
    #[inline]
    pub const fn bishops(self) -> Bitboard {
        self.roles.bishop
    }

    #[inline]
    pub const fn bishops_and_queens(self) -> Bitboard {
        self.bishops().union(self.queens())
    }

    #[inline]
    pub const fn black(self) -> Bitboard {
        self.players.black
    }

    #[inline]
    pub const fn king_of(self, player: Player) -> Option<Square> {
        self.roles.king.intersection(self.player(player)).first()
    }

    #[inline]
    pub const fn kings(self) -> Bitboard {
        self.roles.king
    }

    #[inline]
    pub const fn knights(self) -> Bitboard {
        self.roles.knight
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
    pub(super) const fn play_unchecked(&mut self, player: Player, play: Move) {
        let Some(special) = play.specials() else {
            self.remove(play.from);
            self.remove(play.to);
            self.insert(play.to, play.role.of(player));
            return;
        };

        match special {
            Special::EnPassant => {
                let captured = Square::new(play.to.file(), play.from.rank());
                self.remove(play.from);
                self.remove(captured);
                self.insert(play.to, player.pawn());
            }
            Special::Castle(file) => {
                let side = Side::of_rook(play.from, file);
                let rook_from = player.castle_rook_from(file);
                let rook_to = player.castle_rook_to(side);
                self.remove(play.from);
                self.remove(rook_from);
                self.insert(play.to, player.king());
                self.insert(rook_to, player.rook());
            }
            Special::Promote(role) => {
                self.remove(play.from);
                self.remove(play.to);
                self.insert(play.to, role.of(player));
            }
        }
    }

    #[inline]
    pub const fn player(self, player: Player) -> Bitboard {
        self.players.get(player)
    }

    #[inline]
    pub const fn player_at(self, square: Square) -> Option<Player> {
        match self.get(square) {
            Some(piece) => Some(piece.player),
            None => None,
        }
    }

    #[inline]
    pub const fn queens(self) -> Bitboard {
        self.roles.queen
    }

    #[inline]
    pub const fn role(self, role: Role) -> Bitboard {
        self.roles.get(role)
    }

    #[inline]
    pub const fn role_at(self, square: Square) -> Option<Role> {
        match self.get(square) {
            Some(piece) => Some(piece.role),
            None => None,
        }
    }

    #[inline]
    pub const fn rooks(self) -> Bitboard {
        self.roles.rook
    }

    #[inline]
    pub const fn rooks_and_queens(self) -> Bitboard {
        self.rooks().union(self.queens())
    }

    /// Bishops, rooks and queens.
    #[inline]
    pub const fn sliders(self) -> Bitboard {
        let Roles { bishop, rook, queen, .. } = self.roles;
        bishop.symmetric_difference(rook).symmetric_difference(queen)
    }

    /// Pawns, knights and kings.
    #[inline]
    pub const fn steppers(self) -> Bitboard {
        let Roles { pawn, knight, king, .. } = self.roles;
        pawn.symmetric_difference(knight).symmetric_difference(king)
    }

    #[inline]
    pub const fn unique_king_of(self, player: Player) -> Option<Square> {
        let kings = self.roles.king.intersection(self.player(player));
        if kings.more_than_one() { None } else { kings.first() }
    }

    #[inline]
    pub const fn white(self) -> Bitboard {
        self.players.white
    }
}

/// Board Map API.
impl Board {
    #[inline]
    pub const fn clear(&mut self) {
        *self = Self::EMPTY;
    }

    #[inline]
    pub const fn get(self, square: Square) -> Option<Piece> {
        if !self.occupied.contains(square) {
            return None;
        }

        let player = if self.players.black.contains(square) {
            Black
        } else if self.players.white.contains(square) {
            White
        } else {
            return None;
        };

        let role = if self.roles.pawn.contains(square) {
            Pawn
        } else if self.roles.knight.contains(square) {
            Knight
        } else if self.roles.bishop.contains(square) {
            Bishop
        } else if self.roles.rook.contains(square) {
            Rook
        } else if self.roles.queen.contains(square) {
            Queen
        } else if self.roles.king.contains(square) {
            King
        } else {
            return None;
        };

        Some(role.of(player))
    }

    #[inline]
    pub const fn insert(&mut self, square: Square, piece: Piece) -> Option<Piece> {
        let previous = self.remove(square);
        self.occupied.insert(square);
        self.players.get_mut(piece.player).insert(square);
        self.roles.get_mut(piece.role).insert(square);
        previous
    }

    #[inline]
    pub const fn remove(&mut self, square: Square) -> Option<Piece> {
        let Some(piece) = self.get(square) else {
            return None;
        };
        self.occupied.remove(square);
        self.players.get_mut(piece.player).remove(square);
        self.roles.get_mut(piece.role).remove(square);
        Some(piece)
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl From<Board> for Parts {
    #[inline]
    fn from(board: Board) -> Self {
        board.parts()
    }
}

impl TryFrom<Parts> for Board {
    type Error = Error;

    #[inline]
    fn try_from(parts: Parts) -> Result<Self> {
        Board::new(parts)
    }
}

impl Parts {
    #[inline]
    pub const fn bitor_assign(&mut self, other: Self) {
        self.players.black.append(other.players.black);
        self.players.white.append(other.players.white);

        finite_for!(role in Role {
            self.roles.get_mut(role).append(other.roles.get(role));
        });
    }
}

impl ops::BitOrAssign for Parts {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        Parts::bitor_assign(self, other);
    }
}
