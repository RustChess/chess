use core::{fmt, str};

use crate::{
    Position, Square, finite_for,
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
#[derive(
    Clone, Copy, Debug, DeserializeFromStr, Eq, Ord, PartialEq, PartialOrd, SerializeDisplay,
)]
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

    pub fn from_board(board: Board) -> Option<Self> {
        (0..960).map(Self).find(|scharnagl| scharnagl.board() == board)
    }

    pub fn from_position(position: Position) -> Option<Self> {
        if position.en_passant().is_some()
            || position.round().get() != 1
            || position.reversible() != 0
        {
            return None;
        }

        #[cfg(rust_chess_compute_scharnagl_prefixes)]
        return (0..960).map(Self).find(|&scharnagl| position == Position::freestyle(scharnagl));

        #[cfg(not(rust_chess_compute_scharnagl_prefixes))]
        {
            const PREFIXES: [([u8; 3], Scharnagl); 960] = include!("SCHARNAGL-PREFIXES.rs");
            let prefix = prefix(position);
            let index = PREFIXES.binary_search_by_key(&prefix, |&(prefix, _)| prefix).ok()?;
            let scharnagl = PREFIXES[index].1;
            (position == Position::freestyle(scharnagl)).then_some(scharnagl)
        }
    }

    pub const fn compute_prefixes() -> [([u8; 3], Self); 960] {
        const fn prefix_value(prefix: [u8; 3]) -> u32 {
            u32::from_be_bytes([0, prefix[0], prefix[1], prefix[2]])
        }

        let mut prefixes = [([0; 3], Self(0)); 960];
        let mut i = 0;
        while i < prefixes.len() {
            let scharnagl = Self(i as u16);
            prefixes[i] = (prefix(Position::freestyle(scharnagl)), scharnagl);
            i += 1;
        }

        i = 1;
        while i < prefixes.len() {
            let current = prefixes[i];
            let mut j = i;
            while j > 0 && prefix_value(prefixes[j - 1].0) > prefix_value(current.0) {
                prefixes[j] = prefixes[j - 1];
                j -= 1;
            }
            prefixes[j] = current;
            i += 1;
        }
        prefixes
    }
}

const fn prefix(position: Position) -> [u8; 3] {
    let key = position.key().be_bytes();
    [key[0], key[1], key[2]]
}

impl fmt::Display for Scharnagl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Scharnagl> for u16 {
    fn from(scharnagl: Scharnagl) -> Self {
        scharnagl.0
    }
}

impl str::FromStr for Scharnagl {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let i = s.parse().map_err(|_| "Scharnagl index must be an integer from 0 to 959")?;
        Self::new(i).ok_or("Scharnagl index must be an integer from 0 to 959")
    }
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
    assert_eq!(Scharnagl::from_board(Board::EMPTY), None);
    for i in 0..960 {
        let scharnagl = Scharnagl(i);
        assert_eq!(Scharnagl::from_board(scharnagl.board()), Some(scharnagl));
        assert_eq!(Scharnagl::from_position(Position::freestyle(scharnagl)), Some(scharnagl));
    }

    let position = Position::freestyle(Scharnagl::CHESS);
    assert_eq!(position.board(), Position::start().board());
    assert_eq!(position.castles().get(White, Side::Queen), Some(A));
    assert_eq!(position.castles().get(White, Side::King), Some(H));
    assert_eq!(position.castles().get(Black, Side::Queen), Some(A));
    assert_eq!(position.castles().get(Black, Side::King), Some(H));
}

#[test]
fn freestyle_position_key_prefix() {
    use std::collections::BTreeSet;

    use crate::Position;

    let unique = |len| {
        (0..960)
            .map(|i| Position::freestyle(Scharnagl(i)).key().be_bytes()[..len].to_vec())
            .collect::<BTreeSet<_>>()
            .len()
            == 960
    };
    assert!(!unique(2));
    assert!(unique(3));
}
