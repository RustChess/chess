use core::{fmt, ops};

use crate::{bitboard::Bitboard, finite_for};

use super::{
    File, Move, Piece, Player, Players, Rank, Role, Roles, Scharnagl, Side, Special, Square,
};

// This has 1 + 2 players + 6 roles, so should be 9 * 8 = 72 bytes
//
// The `occupied` field is redundant, at which point it would be 64 bytes or 512 bits.
// Maybe this fits in AVX-512 registers?
//
// Invariant: players disjoint, roles disjoint, both union to same (=occupied)
#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
/// Location of pieces on the board
pub struct Board {
    pub occupied: Bitboard,
    pub players: Players<Bitboard>,
    pub roles: Roles<Bitboard>,
}

/// Piece placement without the redundant occupied cache.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Placement {
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

    pub const fn freestyle(i: Scharnagl) -> Board {
        let backrank = scharnagl(i);
        let mut board = Board::empty();

        finite_for!(file in File {
            let role = backrank[file.index()];
            board.add(Square::new(file, Rank::One), role.of(Player::White));
            board.add(Square::new(file, Rank::Two), Player::White.pawn());
            board.add(Square::new(file, Rank::Seven), Player::Black.pawn());
            board.add(Square::new(file, Rank::Eight), role.of(Player::Black));
        });

        board
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
    pub const fn placement(self) -> Placement {
        Placement { players: self.players, roles: self.roles }
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

    pub const fn add(&mut self, square: Square, piece: Piece) {
        self.occupied.insert(square);
        self.players.get_mut(piece.player).insert(square);
        self.roles.get_mut(piece.role).insert(square);
    }

    pub const fn remove(&mut self, square: Square) -> Option<Piece> {
        let Some(piece) = self.piece_at(square) else {
            return None;
        };
        self.occupied.remove(square);
        self.players.get_mut(piece.player).remove(square);
        self.roles.get_mut(piece.role).remove(square);
        Some(piece)
    }

    pub(super) fn play_unchecked(&mut self, player: Player, play: Move) {
        let Some(special) = play.specials() else {
            self.remove(play.from);
            self.remove(play.to);
            self.add(play.to, play.role.of(player));
            return;
        };

        match special {
            Special::EnPassant => {
                let captured = Square::new(play.to.file(), play.from.rank());
                self.remove(play.from);
                self.remove(captured);
                self.add(play.to, player.pawn());
            }
            Special::Castle(file) => {
                let side = Side::of_rook(play.from, file);
                let rook_from = player.castle_rook_from(file);
                let rook_to = player.castle_rook_to(side);
                self.remove(play.from);
                self.remove(rook_from);
                self.add(play.to, player.king());
                self.add(rook_to, player.rook());
            }
            Special::Promote(role) => {
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

impl Placement {
    #[inline]
    pub const fn board(self) -> Board {
        Board {
            occupied: self.players.black.union_const(self.players.white),
            players: self.players,
            roles: self.roles,
        }
    }
}

impl ops::BitOrAssign for Placement {
    #[inline]
    fn bitor_assign(&mut self, other: Self) {
        self.players.black |= other.players.black;
        self.players.white |= other.players.white;

        finite_for!(role in Role {
            self.roles[role] |= other.roles[role];
        });
    }
}

const fn scharnagl(Scharnagl(mut i): Scharnagl) -> [Role; 8] {
    use Role::*;

    const KNIGHTS: [(u8, u8); 10] =
        [(0, 0), (0, 1), (0, 2), (0, 3), (1, 1), (1, 2), (1, 3), (2, 2), (2, 3), (3, 3)];

    const fn nth_free(roles: &[Role; 8], n: u8) -> File {
        let mut seen = 0;
        finite_for!(file in File {
            if Pawn.eq(roles[file.index()]) {
                if seen == n {
                    return file;
                }
                seen += 1;
            }
        });
        unreachable!()
    }

    let mut roles = [Pawn; 8];

    // Place light bishop on b/d/f/h according to i % 4
    // IOW, last two bits
    let light_bishop = i % 4;
    i /= 4;
    roles[(light_bishop * 2 + 1) as usize] = Bishop;

    // Place dark bishop on a/c/e/g according to i % 4
    // IOW, next two bits
    let dark_bishop = i % 4;
    i /= 4;
    roles[(dark_bishop * 2) as usize] = Bishop;

    // Place queen on remaining files according to i % 6
    // IOW, next six numbers
    let queen = i % 6;
    i /= 6;
    let queen = nth_free(&roles, queen as u8);
    roles[queen.index()] = Queen;

    // There are 960/4/4/6=10 cases left.
    // Place the knights in any two remaining files, using the lookup table
    // of all 2-of-4 subsets with replacement
    let (left_knight, right_knight) = KNIGHTS[i as usize];
    let left_knight = nth_free(&roles, left_knight);
    roles[left_knight.index()] = Knight;
    let right_knight = nth_free(&roles, right_knight);
    roles[right_knight.index()] = Knight;

    // Now fill in the remaining files with rooks and king,
    // ensuring the king is between the rooks
    let rook = nth_free(&roles, 0);
    roles[rook.index()] = Rook;
    let king = nth_free(&roles, 0);
    roles[king.index()] = King;
    let rook = nth_free(&roles, 0);
    roles[rook.index()] = Rook;

    roles
}

#[test]
fn freestyle_positions() {
    use super::{Position, Side};
    use Player::*;

    assert_eq!(Scharnagl::new(960), None);
    assert_eq!(Board::freestyle(Scharnagl(0)).fen(), "bbqnnrkr/pppppppp/8/8/8/8/PPPPPPPP/BBQNNRKR");
    assert_eq!(
        Board::freestyle(Scharnagl(631)).fen(),
        "rnbkqrnb/pppppppp/8/8/8/8/PPPPPPPP/RNBKQRNB"
    );
    assert_eq!(Board::freestyle(Scharnagl(518)), Board::standard());
    assert_eq!(
        Board::freestyle(Scharnagl(959)).fen(),
        "rkrnnqbb/pppppppp/8/8/8/8/PPPPPPPP/RKRNNQBB"
    );

    let position = Position::freestyle(Scharnagl(518));
    assert_eq!(position.board, Position::start().board);
    assert_eq!(position.castles.get(White, Side::Queen), Some(File::A));
    assert_eq!(position.castles.get(White, Side::King), Some(File::H));
    assert_eq!(position.castles.get(Black, Side::Queen), Some(File::A));
    assert_eq!(position.castles.get(Black, Side::King), Some(File::H));
}
