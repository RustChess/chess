use core::fmt;

use crate::{
    Id, Square, finite_for,
    square::{File, Rank},
};

use super::{Board, Player, Role};

use Player::*;
use Rank::*;
use Role::*;

#[cfg(test)]
use File::*;

/// Reinhard Scharnagl's enumeration of all 960 starting positions.
///
/// Standard chess is position 518.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Scharnagl(u16);

impl Scharnagl {
    pub const CHESS: Self = Self(518);

    pub const fn new(i: u16) -> Option<Self> {
        if i < 960 { Some(Self(i)) } else { None }
    }

    pub const fn board(self) -> Board {
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

        let mut i = self.0;
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

        let mut board = Board::EMPTY;
        finite_for!(file in File {
            let role = roles[file.index()];
            board.insert(Square::new(file, One), role.of(White));
            board.insert(Square::new(file, Two), White.pawn());
            board.insert(Square::new(file, Seven), Black.pawn());
            board.insert(Square::new(file, Eight), role.of(Black));
        });
        board
    }
}

impl fmt::Display for Scharnagl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(feature = "const-fn-scharnagl-id")]
// This allow turns the error into a warning, which cannot currently be suppressed.
#[allow(long_running_const_eval)]
// has to be ordered by ID itself
pub static SCHARNAGL_BY_ID: [(Id, Scharnagl); 960] = generate_scharnagl_by_id();
#[cfg(not(feature = "const-fn-scharnagl-id"))]
include!("scharnagl-id.rs");

#[cfg(feature = "const-fn-scharnagl-id")]
const fn generate_scharnagl_by_id() -> [(Id, Scharnagl); 960] {
    let mut table = [(Id(0), Scharnagl(0)); 960];

    let mut i = 0;
    // binary insertion sort
    while i < 960 {
        let scharnagl = Scharnagl(i as u16);
        let entry = (Board::freestyle(scharnagl).standard_id(), scharnagl);

        let mut j = i;
        while j > 0 && entry.0.0 < table[j - 1].0.0 {
            table[j] = table[j - 1];
            j -= 1;
        }
        table[j] = entry;
        i += 1;
    }

    table
}

pub fn scharnagl_by_id(id: Id) -> Option<Scharnagl> {
    SCHARNAGL_BY_ID.binary_search_by_key(&id, |(id, _)| *id).ok().map(|i| SCHARNAGL_BY_ID[i].1)
}

#[test]
fn freestyle_positions() {
    use crate::{Position, Side};

    assert_eq!(Scharnagl::new(960), None);
    assert_eq!(Board::freestyle(Scharnagl(0)).fen(), "bbqnnrkr/pppppppp/8/8/8/8/PPPPPPPP/BBQNNRKR");
    assert_eq!(
        Board::freestyle(Scharnagl(631)).fen(),
        "rnbkqrnb/pppppppp/8/8/8/8/PPPPPPPP/RNBKQRNB"
    );
    assert_eq!(Board::freestyle(Scharnagl::CHESS), Board::standard());
    assert_eq!(
        Board::freestyle(Scharnagl(959)).fen(),
        "rkrnnqbb/pppppppp/8/8/8/8/PPPPPPPP/RKRNNQBB"
    );

    let position = Position::freestyle(Scharnagl::CHESS);
    assert_eq!(position.board(), Position::start().board());
    assert_eq!(position.castles().get(White, Side::Queen), Some(A));
    assert_eq!(position.castles().get(White, Side::King), Some(H));
    assert_eq!(position.castles().get(Black, Side::Queen), Some(A));
    assert_eq!(position.castles().get(Black, Side::King), Some(H));
}
