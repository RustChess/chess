// https://www.chessprogramming.org/Zobrist_Hashing
//
// Idea is to:
// - use a 32 byte seed
// - use 128 bit pseudorandom numbers
// - generate the 12 * 64 + 1 + 4 + 8 = 781 instances with ChaCha20Rng

use chacha20::{ChaCha20Rng, rand_core::SeedableRng as _};

use super::{Board, Map, Player, Players, Position, Role, Side, Sides, Square, en_passant};

/// 32 byte seed, uniquely determines an [`Identifier`]
pub type Seed = [u8; 32];

/// Zobrist hash based ID, determined by an [`Identifier`]
pub trait Id {
    fn id(self, i: Identifier) -> u128;
}

trait Random {
    fn random(self) -> u128;
}

impl Random for &mut ChaCha20Rng {
    fn random(self) -> u128 {
        use chacha20::rand_core::RngCore as _;
        let mut random = [0u8; 16];
        self.fill_bytes(&mut random);
        u128::from_le_bytes(random)
    }
}

impl Id for Player {
    fn id(self, i: Identifier) -> u128 {
        if let Player::Black = self { i.black } else { 0 }
    }
}

impl Id for Players<Sides> {
    fn id(self, i: Identifier) -> u128 {
        let mut id = 0;
        for player in Player::ALL {
            for side in Side::ALL {
                if self[player][side] {
                    id ^= i.castle[player][side];
                }
            }
        }
        id
    }
}

impl Id for Option<en_passant::Square> {
    fn id(self, i: Identifier) -> u128 {
        self.map(|square| i.en_passant[square]).unwrap_or_default()
    }
}

impl Id for Board {
    fn id(self, i: Identifier) -> u128 {
        let mut id = 0;
        for square in Square::iter() {
            id ^= self
                .piece_at(square)
                .map(|piece| i.square_piece[square][piece.role][piece.player])
                .unwrap_or_default();
        }
        id
    }
}

impl<V> Id for Position<V> {
    fn id(self, i: Identifier) -> u128 {
        let mut id = self.turn.id(i);
        id ^= self.board.id(i);
        id ^= self.castle.id(i);
        id ^= self.en_passant.id(i);
        id
    }
}

/// 781 unsigned 128-bit integers that determine a unique 128-bit ID for every chess position
#[derive(Clone, Copy, Debug)]
pub struct Identifier {
    black: u128,
    castle: Map<Player, Map<Side, u128, 2>, 2>,
    // TODO: Make these en_passant::Squares<T>  and Pieces<T> types
    // that are actually arrays or structs so everything becomes Copy
    en_passant: Map<en_passant::Square, u128, 16>,
    square_piece: Map<Square, Map<Role, Map<Player, u128, 2>, 6>, 64>,
}

impl Identifier {
    /// identify a value of type `T` by its ID
    pub fn id<T: Id>(self, value: T) -> u128 {
        value.id(self)
    }
}

impl From<Seed> for Identifier {
    fn from(seed: Seed) -> Self {
        Self::new(seed)
    }
}

impl Identifier {
    pub fn new(seed: Seed) -> Self {
        let rng = &mut ChaCha20Rng::from_seed(seed);

        let black: u128 = rng.random();

        let mut castle = Map::default();
        for player in Player::ALL {
            let mut players = Map::default();
            for side in Side::ALL {
                players[side] = rng.random();
            }
            castle[player] = players;
        }

        let mut en_passant = Map::default();
        for square in en_passant::Square::ALL {
            en_passant[square] = rng.random();
        }

        let mut square_piece = Map::default64();
        for square in Square::iter() {
            let mut squares = Map::default();
            for role in Role::ALL {
                let mut roles = Map::default();
                for player in Player::ALL {
                    roles[player] = rng.random();
                }
                squares[role] = roles;
            }
            square_piece[square] = squares;
        }

        Identifier { black, castle, en_passant, square_piece }
    }
}

#[test]
fn example() {
    use crate::formats::{Parser as _, fen};

    // This is a bad seed!
    // OTOH it's canonical so who knows if it's good for our purpose.
    let i = Identifier::new([0u8; 32]);

    let position: Position<_> =
        fen::position_partial_fen.parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR").unwrap();
    println!("{}", position.id(i));

    let position: Position<_> =
        fen::position_partial_fen.parse("rnbqkbnr/ppp1pppp/8/8/8/8/PPPPPPPP/RNBQKBNR").unwrap();
    println!("{}", position.id(i));
}
