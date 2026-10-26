pub mod base58;
#[cfg(any(rust_chess_compile_time_polyglot_basis, test))]
pub mod polyglot;
#[cfg(any(rust_chess_compile_time_standard_basis, test))]
pub mod standard;

use crate::{
    board::{Board, Player, PlayerTable, Role, RoleTable},
    finite::Empty as _,
    game::{Game, Line},
    position::{Castles, EnPassant, EnPassantTable, Move, Position, Side},
    square::{FileTable, SquareTable},
};

/// Type of chess lookup keys.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Key(pub u128);

// Our standard basis has 802 consistent keys:
// - board: 64 * 2 * 6 = 768
// - turn: 2, castle: 2 * 8 = 16, en-passant: 16 squares
//
// Polyglot basis has 781 "random" keys:
// - pieces: 12 * 64 = 768
// - side to move: 1, castling: 4, en-passant files: 8
// We set zeros and repeated values to achieve compatibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Basis {
    pub board: BoardTable<Key>,
    pub turn: PlayerTable<Key>,
    pub castle: CastleTable<Key>,
    pub en_passant: EnPassantTable<Key>,
}

impl Position {
    // our 128-bit replacement of the classical Polyglot hash
    pub const fn key(self) -> Key {
        self.key_with_basis(Basis::standard())
    }

    // the classical Polyglot hash, just embedded into u128
    pub const fn polyglot_key(self) -> Key {
        self.key_with_basis(Basis::polyglot())
    }

    pub const fn key_with_basis(self, basis: &Basis) -> Key {
        // In validated positions, en_passant is normalized to effective rights.
        self.apparent_key(basis)
            .xor(castle_key(basis, self.castles()))
            .xor(en_passant_key(basis, self.en_passant()))
    }

    // What one typically sees in a depicted position: The board, and the player to move
    pub const fn apparent_key(self, basis: &Basis) -> Key {
        self.board().key_with_basis(basis).xor(turn_key(basis, self.turn()))
    }

    // standard key plus counters
    pub fn full_key(self) -> Key {
        self.key()
            .xor(counter_key("reversible", self.reversible()))
            .xor(counter_key("round", self.round().get()))
    }
}

impl Board {
    pub const fn polyglot_key(self) -> Key {
        self.key_with_basis(Basis::polyglot())
    }

    pub const fn key(self) -> Key {
        self.key_with_basis(Basis::standard())
    }

    pub const fn key_with_basis(self, basis: &Basis) -> Key {
        let mut key = Key(0);

        finite_for!(player in Player {
            finite_for!(role in Role {
                let mut squares = self.player(player).intersection(self.role(role));
                while let Some(square) = squares.pop_first() {
                    key = key.xor(basis.board.get(square).get(player).get(role));
                }
            });
        });

        key
    }
}

impl Game {
    pub fn mainline_key(&self) -> Key {
        let mut position = self.start();
        line_key(
            position.position(),
            core::iter::from_fn(|| {
                let play = position.main().ok()?;
                position = play.position();
                Some(play.play())
            }),
        )
    }

    pub fn mainline_keys(&self) -> Vec<Key> {
        let mut position = self.start();
        let mut hash = line_hash(position.position());
        let mut keys = vec![Key(fold(hash.finalize()))];
        while let Ok(play) = position.main() {
            hash = line_hash_play(hash, play.play());
            keys.push(Key(fold(hash.finalize())));
            position = play.position();
        }
        keys
    }
}

impl Line<Position, Move> {
    pub fn key(&self) -> Key {
        line_key(self.position, self.moves.iter().copied())
    }
}

fn line_key(position: Position, moves: impl IntoIterator<Item = Move>) -> Key {
    let mut hash = line_hash(position);
    for play in moves {
        hash = line_hash_play(hash, play);
    }
    Key(fold(hash.finalize()))
}

fn line_hash(position: Position) -> sha2_const::Sha256 {
    sha2_const::Sha256::new().update(b"line:").update(position.key().to_string().as_bytes())
}

fn line_hash_play(hash: sha2_const::Sha256, play: Move) -> sha2_const::Sha256 {
    hash.update(b":").update(play.to_string().as_bytes())
}

impl Key {
    pub const fn xor(self, other: Key) -> Key {
        Key(self.0 ^ other.0)
    }

    pub const fn u128(self) -> u128 {
        self.0
    }

    pub const fn be_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }

    pub fn nonce() -> Self {
        let nonce = base58::Base58::random_str(12);
        let bytes = nonce.as_bytes();
        let mut key = [0; 16];
        let start = key.len() - bytes.len();
        key[start..].copy_from_slice(bytes);
        Key(u128::from_be_bytes(key))
    }
}

type U256 = [u8; 32];

pub const fn hash(bytes: &[u8]) -> Key {
    Key(fold(sha256(bytes)))
}

const fn sha256(bytes: &[u8]) -> U256 {
    sha2_const::Sha256::new().update(bytes).finalize()
}

const fn fold(hash: U256) -> u128 {
    let (upper, lower) = split(hash);
    upper ^ lower
}

const fn split(hash: U256) -> (u128, u128) {
    let mut upper = [0u8; 16];
    let mut lower = [0u8; 16];

    let mut i = 0;
    while i < 16 {
        upper[i] = hash[i];
        lower[i] = hash[i + 16];
        i += 1;
    }

    (u128::from_be_bytes(upper), u128::from_be_bytes(lower))
}

type BoardTable<T> = SquareTable<PlayerTable<RoleTable<T>>>;
type CastleTable<T> = PlayerTable<FileTable<T>>;

impl Basis {
    pub const fn empty() -> Self {
        Self {
            board: BoardTable::EMPTY,
            turn: PlayerTable::EMPTY,
            castle: CastleTable::EMPTY,
            en_passant: EnPassantTable::EMPTY,
        }
    }

    pub const fn polyglot() -> &'static Self {
        #[cfg(rust_chess_compile_time_polyglot_basis)]
        static BASIS: Basis = polyglot::compute_basis();
        #[cfg(not(rust_chess_compile_time_polyglot_basis))]
        static BASIS: Basis = include!("key/POLYGLOT-BASIS.rs");

        &BASIS
    }

    pub const fn standard() -> &'static Self {
        #[cfg(rust_chess_compile_time_standard_basis)]
        #[allow(long_running_const_eval)]
        static BASIS: Basis = standard::compute_table();
        #[cfg(not(rust_chess_compile_time_standard_basis))]
        static BASIS: Basis = include!("key/STANDARD-BASIS.rs");

        &BASIS
    }
}

fn counter_key(prefix: &str, counter: u32) -> Key {
    hash(format!("{prefix}:{counter}").as_bytes())
}

const fn turn_key(basis: &Basis, turn: Player) -> Key {
    basis.turn.get(turn)
}

const fn en_passant_key(basis: &Basis, en_passant: Option<EnPassant>) -> Key {
    match en_passant {
        Some(square) => basis.en_passant.get(square),
        None => Key(0),
    }
}

const fn castle_key(basis: &Basis, castles: Castles) -> Key {
    let mut key = Key(0);

    finite_for!(player in Player {
        finite_for!(side in Side {
            if let Some(file) = castles.get(player, side) {
                key = key.xor(basis.castle.get(player).get(file));
            }
        });
    });

    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Move, board::Role::*, game::Cursor, square::Square::*};

    const COLLISION_FEN: &str = "2b1kqr1/p2p3p/3p4/p2PpP2/PpP2p2/6P1/8/RRB1KQ1N w - - 21 32";
    const COLLISION_GAME: &str = "
        1. e4 c5 2. Ba6 bxa6 3. b4 g6 4. f4 Bh6 5. f5 Bf4 6. g3 Nf6
        7. gxf4 Nh5 8. c4 cxb4 9. Ne2 Ng3 10. Nd4 a5 11. hxg3 Nc6
        12. Nxc6 Rb8 13. Nxb8 g5 14. d4 f6 15. Na6 gxf4 16. Nc5 Rg8
        17. Nb7 Rh8 18. Nd6+ exd6 19. e5 fxe5 20. d5 Qf6 21. a4 Rg8
        22. Nd2 Qf7 23. Rb1 Qf6 24. Ne4 Qf7 25. Rh2 Qf6 26. Nf2 Qf7
        27. Nh1 Qf6 28. Qd3 Qf7 29. Qf1 Qf6 30. Ra2 Qf7 31. Raa1 Qf8 *";
    const START: Position = Position::start();

    #[test]
    fn document_line_keys() {
        let d4 = Move::normal(Pawn, D2, D4);
        let e4 = Move::normal(Pawn, E2, E4);
        let e5 = Move::normal(Pawn, E7, E5);
        let c5 = Move::normal(Pawn, C7, C5);
        let nf3 = Move::normal(Knight, G1, F3);
        let key =
            |moves: &[Move]| Line { position: START, moves: moves.to_vec() }.key().to_string();

        assert_eq!(key(&[]), "Y4LCEFvKgeZxrQoFJeeQZ8");
        assert_eq!(key(&[e4]), "95asNCfiG3MeWa1q5jsZ1n");
        assert_eq!(key(&[e4, c5]), "JQt9mENjV4RUbvog6oWGtb");
        assert_eq!(key(&[e4, e5, nf3]), "LR2zFDKgxYWDcLHp9nME2i");
        assert_eq!(key(&[nf3, d4]), "75SZLN4dvpcbErRJaDcnKR");
    }

    #[test]
    fn document_game_mainline_keys() {
        use crate::formats::{Parser as _, pgn};

        // start position, no moves
        let game = Game::chess(START).unwrap();
        let key = game.mainline_key();
        assert_eq!(key.to_string(), "Y4LCEFvKgeZxrQoFJeeQZ8");

        // 1. e4 on start position
        let mut cursor = Cursor::new(game);
        let e4 = Move::normal(Pawn, E2, E4);
        let e5 = Move::normal(Pawn, E7, E5);
        cursor.option_mut_or_push(e4).unwrap();
        let e4_game = cursor.game().clone();
        assert_eq!(e4_game.mainline_key().to_string(), "95asNCfiG3MeWa1q5jsZ1n");

        cursor.option_mut_or_push(e5).unwrap();
        assert_eq!(
            cursor.game().mainline_keys(),
            [
                Line { position: START, moves: vec![] }.key(),
                Line { position: START, moves: vec![e4] }.key(),
                Line { position: START, moves: vec![e4, e5] }.key(),
            ],
        );

        // The weird game
        let pgn = pgn::pgn.parse(COLLISION_GAME).unwrap();
        let game: Game = pgn.try_into().unwrap();
        assert_eq!(game.mainline_key().to_string(), "FtnuJA6Hk95K9GM4Ay8yN3");
    }

    #[test]
    fn document_polyglot_start_position_hash() {
        assert_eq!(START.polyglot_key(), Key(0x463b_9618_1691_fc9c));
    }

    #[test]
    fn document_polyglot_start_position_collision() {
        // Polyglot collision with the standard start position:
        // https://talkchess.com/viewtopic.php?sid=19ffa9bbce9b0b8c00e176365ba29da6&start=20&t=57255
        // https://talkchess.com/viewtopic.php?start=40&t=57255
        let start = Position::start();
        let position = Position::from_fen(COLLISION_FEN).unwrap();

        assert_eq!(position.polyglot_key(), start.polyglot_key());
        assert_ne!(position.key(), start.key());
    }

    #[test]
    fn reach_polyglot_collision_position_from_linked_movetext() {
        use crate::formats::{Parser as _, pgn};

        // The first TalkChess thread referenced by
        // `documents_polyglot_start_position_collision` gives this game
        // reaching the collision position.
        let pgn = pgn::pgn.parse(COLLISION_GAME).unwrap();
        let game: Game = pgn.try_into().unwrap();
        let mut cursor = Cursor::new(game);
        cursor.mainline_end();
        let constructed = cursor.position();

        let expected = Position::from_fen(COLLISION_FEN).unwrap();

        assert_eq!(constructed.fen(), expected.fen());
        assert_eq!(constructed.transposition_fen(), expected.transposition_fen());
        assert_eq!(constructed.key(), expected.key());
        assert_eq!(constructed.polyglot_key(), expected.polyglot_key());
    }

    #[test]
    fn document_polyglot_zero_hash_position() {
        // Polyglot zero-hash position:
        // https://talkchess.com/forum/viewtopic.php?p=482951
        // https://talkchess.com/viewtopic.php?sid=19ffa9bbce9b0b8c00e176365ba29da6&start=20&t=57255
        let position =
            Position::from_fen("2b1k3/4p3/3p1p2/p2P2p1/P2P4/2P2PP1/4P3/2NQKB2 b - - 0 1").unwrap();

        assert_eq!(position.polyglot_key(), Key(0));
        assert_ne!(position.key_with_basis(Basis::standard()), Key(0));
    }
}
