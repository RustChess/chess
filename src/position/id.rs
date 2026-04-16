// https://www.chessprogramming.org/Zobrist_Hashing
//
// The basis can be chosen
// - either pseudo-randomly, from a given seed (using ChaCha20Rng),
// - or deterministically, hashing natural descriptions of the basis elements

use std::cell::LazyCell;

use rand::{
    Rng, RngExt as _, SeedableRng as _,
    distr::{Distribution, StandardUniform},
    rngs::ChaCha20Rng,
};

#[allow(clippy::declare_interior_mutable_const)]
pub const BASIS: LazyCell<Basis> = LazyCell::new(Basis::default);

use super::{Board, Map, Player, Players, Position, Role, Side, Sides, Square, en_passant};

/// 32 byte seed, uniquely determines a [`Basis`]
pub type Seed = [u8; 32];

impl<V> Position<V> {
    pub fn id(self) -> u128 {
        let basis = BASIS;
        <Self as Id>::id(self, *basis)
    }
}

/// Zobrist hash based ID, determined by a [`Basis`]
pub trait Id {
    fn id(self, i: Basis) -> u128;
}

/// 781 unsigned 128-bit integers that determine a unique 128-bit ID for every chess position
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Basis {
    board: Map<Square, Map<Role, Map<Player, u128, 2>, 6>, 64>,
    black: u128,
    castle: Map<Player, Map<Side, u128, 2>, 2>,
    en_passant: Map<en_passant::Square, u128, 16>,
}

impl Id for Player {
    fn id(self, basis: Basis) -> u128 {
        if let Player::Black = self { basis.black } else { 0 }
    }
}

impl Id for Players<Sides> {
    fn id(self, basis: Basis) -> u128 {
        let mut id = 0;
        for player in Player::ALL {
            for side in Side::ALL {
                if self[player][side] {
                    id ^= basis.castle[player][side];
                }
            }
        }
        id
    }
}

impl Id for Option<en_passant::Square> {
    fn id(self, basis: Basis) -> u128 {
        self.map(|square| basis.en_passant[square]).unwrap_or_default()
    }
}

impl Id for Board {
    fn id(self, basis: Basis) -> u128 {
        let mut id = 0;
        for square in Square::iter() {
            id ^= self
                .piece_at(square)
                .map(|piece| basis.board[square][piece.role][piece.player])
                .unwrap_or_default();
        }
        id
    }
}

impl<V> Id for Position<V> {
    fn id(self, basis: Basis) -> u128 {
        let mut id = self.turn.id(basis);
        id ^= self.board.id(basis);
        id ^= self.castle.id(basis);
        id ^= self.en_passant.id(basis);
        id
    }
}

impl Default for Basis {
    fn default() -> Self {
        let black = hash(b"black");

        let mut castle = Map::default();
        for player in Player::ALL {
            let player_string = player.to_string();

            let mut players = Map::default();
            for side in Side::ALL {
                players[side] =
                    fold(sha256_2(player_string.as_bytes(), side.to_string().as_bytes()));
            }
            castle[player] = players;
        }

        let mut en_passant = Map::default();
        for square in en_passant::Square::ALL {
            en_passant[square] = fold(sha256(square.to_string().as_bytes()));
        }

        let mut board = Map::default64();
        for square in Square::iter() {
            let square_string = square.to_string();
            let mut squares = Map::default();
            for role in Role::ALL {
                let role_string = role.to_string();
                let mut roles = Map::default();
                for player in Player::ALL {
                    roles[player] = fold(sha256_3(
                        square_string.as_bytes(),
                        role_string.as_bytes(),
                        player.to_string().as_bytes(),
                    ));
                }
                squares[role] = roles;
            }
            board[square] = squares;
        }

        Basis { black, castle, en_passant, board }
    }
}

impl Distribution<Basis> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Basis {
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

        let mut board = Map::default64();
        for square in Square::iter() {
            let mut squares = Map::default();
            for role in Role::ALL {
                let mut roles = Map::default();
                for player in Player::ALL {
                    roles[player] = rng.random();
                }
                squares[role] = roles;
            }
            board[square] = squares;
        }

        Basis { black, castle, en_passant, board }
    }
}

impl Basis {
    /// identify a value of type `T` by its ID
    pub fn id<T: Id>(self, value: T) -> u128 {
        value.id(self)
    }
}

impl From<Seed> for Basis {
    fn from(seed: Seed) -> Self {
        Self::new(seed)
    }
}

impl Basis {
    pub fn new(seed: Seed) -> Self {
        let mut rng = ChaCha20Rng::from_seed(seed);
        rng.random()
    }
}

const fn sha256(bytes: &[u8]) -> [u8; 32] {
    sha2_const::Sha256::new().update(bytes).finalize()
}

#[cfg(test)]
#[allow(dead_code)]
fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap()
}

#[cfg(test)]
#[allow(dead_code)]
fn json_pretty(value: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap()
}

const fn sha256_2(bytes1: &[u8], bytes2: &[u8]) -> [u8; 32] {
    sha2_const::Sha256::new().update(bytes1).update(bytes2).finalize()
}

const fn sha256_3(bytes1: &[u8], bytes2: &[u8], bytes3: &[u8]) -> [u8; 32] {
    sha2_const::Sha256::new().update(bytes1).update(bytes2).update(bytes3).finalize()
}

const fn fold(bytes: [u8; 32]) -> u128 {
    let mut upper = [0u8; 16];
    upper[0x0] = bytes[0x0];
    upper[0x1] = bytes[0x1];
    upper[0x2] = bytes[0x2];
    upper[0x3] = bytes[0x3];
    upper[0x4] = bytes[0x4];
    upper[0x5] = bytes[0x5];
    upper[0x6] = bytes[0x6];
    upper[0x7] = bytes[0x7];
    upper[0x8] = bytes[0x8];
    upper[0x9] = bytes[0x9];
    upper[0xA] = bytes[0xA];
    upper[0xB] = bytes[0xB];
    upper[0xC] = bytes[0xC];
    upper[0xD] = bytes[0xD];
    upper[0xE] = bytes[0xE];
    upper[0xF] = bytes[0xF];

    let mut lower = [0u8; 16];
    lower[0x0] = bytes[0x10];
    lower[0x1] = bytes[0x11];
    lower[0x2] = bytes[0x12];
    lower[0x3] = bytes[0x13];
    lower[0x4] = bytes[0x14];
    lower[0x5] = bytes[0x15];
    lower[0x6] = bytes[0x16];
    lower[0x7] = bytes[0x17];
    lower[0x8] = bytes[0x18];
    lower[0x9] = bytes[0x19];
    lower[0xA] = bytes[0x1A];
    lower[0xB] = bytes[0x1B];
    lower[0xC] = bytes[0x1C];
    lower[0xD] = bytes[0x1D];
    lower[0xE] = bytes[0x1E];
    lower[0xF] = bytes[0x1F];

    let upper = u128::from_be_bytes(upper);
    let lower = u128::from_be_bytes(lower);
    upper ^ lower
}

pub const fn hash(bytes: &[u8]) -> u128 {
    fold(sha256(bytes))
}

#[test]
fn default_id() {
    use crate::formats::{Parser as _, fen};

    // This is a bad seed!
    // OTOH it's canonical so who knows if it's good for our purpose.
    // let basis = Basis::new([0u8; 32]);

    let basis = BASIS;

    println!("{}", json_pretty(&*basis));

    let position: Position<_> =
        fen::position_partial_fen.parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR").unwrap();
    println!("{}", position.id());

    let position: Position<_> =
        fen::position_partial_fen.parse("rnbqkbnr/ppp1pppp/8/8/8/8/PPPPPPPP/RNBQKBNR").unwrap();
    println!("{}", position.id());

    // panic!();
}
