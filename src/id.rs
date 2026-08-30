pub mod base58;
pub mod basis;
pub use basis::{Basis, POLYGLOT, STANDARD};

use crate::{
    game::Game,
    position::{Board, Player, Players, Position, Role, Side, Sides, Variant, en_passant},
    variant::{Chess, Freestyle},
};

/// Type of globally unique identifiers.
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Id(pub u128);

impl Id {
    pub const fn xor(self, other: Id) -> Id {
        Id(self.0 ^ other.0)
    }

    pub const fn u128(self) -> u128 {
        self.0
    }

    pub fn nonce() -> Self {
        let nonce = base58::Base58::random_str(12);
        let bytes = nonce.as_bytes();
        let mut id = [0; 16];
        let start = id.len() - bytes.len();
        id[start..].copy_from_slice(bytes);
        Id(u128::from_be_bytes(id))
    }
}

type U256 = [u8; 32];

pub const fn hash(bytes: &[u8]) -> Id {
    Id(fold(sha256(bytes)))
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

impl Game {
    // Experimental: A globally unique ID for all games
    pub fn id(&self) -> Id {
        use crate::game::cursor::Mainline;

        let mut id = hash(format!("game:{}", self.start().id()).as_bytes());

        for play in Mainline::new(self) {
            id = hash(format!("game:{id}:{}", play.play().uci()).as_bytes());
        }

        id
    }

    // // TODO: This is probably a bad idea
    // //
    // // start:2iHiqJgL4hH1Qqqng6VaDF:move:e2e4 => AipYeC9ie6GSVAKwJsVkvB
    // // play:MSVFqF2aDcnN8CziQZQmqY:move:e7e5 => J3bJ2SruB518B2qn9QKKgs
    // // play:8pxD8BKK3A9XZuDzrcuexd:move:g1f3 => 26BuaaRqhKHZVs9YRHvLzb
    // // play:4r2FcZ9jEJukZ3wEGPn3h:move:e7e8q => C6VatJhT8ZjDrsToNGJh4N
    // fn play_id(&self, previous: Option<Id>, play: Move) -> Id {
    //     if let Some(previous) = previous {
    //         hash(format!("play:{previous}:move:{}", play.uci()).as_bytes())
    //     } else {
    //         hash(format!("start:{}:move:{}", self.start().id(), play.uci()).as_bytes())
    //     }
    // }
}

impl Board {
    pub const fn polyglot_id(self) -> Id {
        self.id(&POLYGLOT)
    }

    pub const fn standard_id(self) -> Id {
        self.id(&STANDARD)
    }

    pub const fn id(self, basis: &Basis) -> Id {
        let mut id = Id(0);

        let mut p = 0;
        while p < Player::LEN as u8 {
            let player = Player::panicky_from_index(p);

            let mut r = 0;
            while r < Role::LEN as u8 {
                let role = Role::panicky_from_index(r);

                let mut squares = self.players.get(player).intersection_const(self.roles.get(role));
                while let Some(square) = squares.pop_first() {
                    id = id.xor(basis.board.get(square).get(player).get(role));
                }
                r += 1;
            }
            p += 1;
        }
        id
    }
}

impl<V: VariantId> Position<V> {
    // standard ID plus counters
    pub fn id(self) -> Id {
        self.standard_id()
            .xor(counter_id("reversible", self.reversible))
            .xor(counter_id("round", self.round.get()))
    }

    // our 128-bit replacement of the classical Polyglot hash
    pub const fn standard_id(self) -> Id {
        self.transposition_id(&STANDARD).xor(V::ID)
    }

    // the classical Polyglot hash, just embedded into u128
    pub const fn polyglot_id(self) -> Id {
        self.transposition_id(&POLYGLOT)
    }

    pub const fn transposition_id(self, basis: &Basis) -> Id {
        self.apparent_id(basis)
            .xor(castle_id(basis, self.castle))
            .xor(en_passant_id(basis, self.effective_en_passant()))
    }

    // What one typically sees in a depicted position: The board, and the player to move
    pub const fn apparent_id(self, basis: &Basis) -> Id {
        self.board.id(basis).xor(turn_id(basis, self.turn))
    }
}

fn counter_id(prefix: &str, counter: u32) -> Id {
    hash(format!("{prefix}:{counter}").as_bytes())
}

const fn turn_id(basis: &Basis, turn: Player) -> Id {
    basis.turn.get(turn)
}

const fn en_passant_id(basis: &Basis, en_passant: Option<en_passant::Square>) -> Id {
    match en_passant {
        Some(square) => basis.en_passant.square.get(square),
        None => basis.en_passant.none,
    }
}

const fn castle_id(basis: &Basis, castle: Players<Sides>) -> Id {
    let mut id = Id(0);

    let mut p = 0;
    while p < Player::LEN as u8 {
        let player = Player::panicky_from_index(p);

        let mut s = 0;
        while s < Side::LEN as u8 {
            let side = Side::panicky_from_index(s);
            if castle.get(player).get(side) {
                id = id.xor(basis.castle.get(player).get(side));
            }
            s += 1;
        }
        p += 1;
    }

    id
}

pub trait VariantId: Variant {
    const ID: Id;
}

impl VariantId for Chess {
    const ID: Id = STANDARD.variant.chess;
}

impl VariantId for Freestyle {
    const ID: Id = STANDARD.variant.freestyle;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Move, Square, game::Cursor, variant::Chess};

    const COLLISION_FEN: &str = "2b1kqr1/p2p3p/3p4/p2PpP2/PpP2p2/6P1/8/RRB1KQ1N w - - 21 32";
    const COLLISION_GAME: &str = "
        1. e4 c5 2. Ba6 bxa6 3. b4 g6 4. f4 Bh6 5. f5 Bf4 6. g3 Nf6
        7. gxf4 Nh5 8. c4 cxb4 9. Ne2 Ng3 10. Nd4 a5 11. hxg3 Nc6
        12. Nxc6 Rb8 13. Nxb8 g5 14. d4 f6 15. Na6 gxf4 16. Nc5 Rg8
        17. Nb7 Rh8 18. Nd6+ exd6 19. e5 fxe5 20. d5 Qf6 21. a4 Rg8
        22. Nd2 Qf7 23. Rb1 Qf6 24. Ne4 Qf7 25. Rh2 Qf6 26. Nf2 Qf7
        27. Nh1 Qf6 28. Qd3 Qf7 29. Qf1 Qf6 30. Ra2 Qf7 31. Raa1 Qf8 *";
    const START: Position<Chess> = Position::standard();

    #[test]
    fn documents_game_ids() {
        use crate::formats::{Parser as _, pgn};

        // start position, no moves
        let game = Game::new(START);
        let id = game.id();
        assert_eq!(id.to_string(), "6e5hnKuS5zmzFcBBsSUzXp");
        assert_eq!(id.u128(), 60703719936619539482143532120728486023);

        // 1. e4 on start position
        let mut cursor = Cursor::new(game);
        cursor.push(Move::normal(Role::Pawn, Square::E2, Square::E4)).unwrap();
        let e4 = cursor.into_inner();
        assert_eq!(e4.id().to_string(), "RJBYu6tEFvwRaRZ4au4o6F");
        assert_eq!(e4.id().u128(), 261533259436473337201551646082583728576);

        // The weird game
        let pgn = pgn::game.parse(COLLISION_GAME).unwrap();
        let game: Game = pgn.try_into().unwrap();
        assert_eq!(game.id().to_string(), "ChJouM3wB4bbzRarRDA3nf");
        assert_eq!(game.id().u128(), 125888540804255007175134348310965693064);
    }

    #[test]
    fn documents_polyglot_start_position_hash() {
        assert_eq!(START.polyglot_id(), Id(0x463b_9618_1691_fc9c));
    }

    #[test]
    fn documents_polyglot_start_position_collision() {
        // Polyglot collision with the standard start position:
        // https://talkchess.com/viewtopic.php?sid=19ffa9bbce9b0b8c00e176365ba29da6&start=20&t=57255
        // https://talkchess.com/viewtopic.php?start=40&t=57255
        let start = Position::<Chess>::standard();
        let position = Position::<Chess>::from_fen(COLLISION_FEN).unwrap();

        assert_eq!(position.polyglot_id(), start.polyglot_id());
        assert_ne!(position.standard_id(), start.standard_id());
    }

    #[test]
    fn reaches_polyglot_collision_position_from_linked_movetext() {
        use crate::formats::{Parser as _, pgn};

        // The first TalkChess thread referenced by
        // `documents_polyglot_start_position_collision` gives this game
        // reaching the collision position.
        let pgn = pgn::game.parse(COLLISION_GAME).unwrap();
        let game: Game = pgn.try_into().unwrap();
        let mut cursor = Cursor::new(game);
        cursor.end();
        let constructed = cursor.position();

        let expected = Position::<Chess>::from_fen(COLLISION_FEN).unwrap();

        assert_eq!(constructed.fen(), expected.fen());
        assert_eq!(constructed.transposition_fen(), expected.transposition_fen());
        assert_eq!(constructed.standard_id(), expected.standard_id());
        assert_eq!(constructed.polyglot_id(), expected.polyglot_id());
    }

    #[test]
    fn documents_polyglot_zero_hash_position() {
        // Polyglot zero-hash position:
        // https://talkchess.com/forum/viewtopic.php?p=482951
        // https://talkchess.com/viewtopic.php?sid=19ffa9bbce9b0b8c00e176365ba29da6&start=20&t=57255
        let position =
            Position::<Chess>::from_fen("2b1k3/4p3/3p1p2/p2P2p1/P2P4/2P2PP1/4P3/2NQKB2 b - - 0 1")
                .unwrap();

        assert_eq!(position.polyglot_id(), Id(0));
        assert_ne!(position.transposition_id(&STANDARD), Id(0));
    }
}
