pub mod base58;
pub mod basis;
pub use basis::{Basis, POLYGLOT, STANDARD};

// Our "standard basis" has 804 "consistent" Ids:
// - board: 64 * 2 * 6 = 768
// - turn: 2
// - castle: 2 * 8 = 16
// - en-passant: 16 squares
// - variant: 2
//
// Polyglot basis has 781 "random" IDs:
// - pieces: 12 * 64 = 768
// - side to move: 1
// - castling: 4
// - en-passant files: 8
// We set zeros/repeat to achieve compatibility.

use crate::{
    finite_for,
    game::Game,
    position::{Board, Castles, EnPassant, Player, Position, Role, Side, VariantEnum},
    variant::{Unvalidated, Validate, Variant},
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
    // TODO: Make this variant-aware before `Game<Freestyle>` uses it.
    // The current hash uses standard UCI formatting for moves.
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

impl<V: Validate> Position<V> {
    // standard ID plus counters and explicit variant
    pub fn id(self) -> Id {
        self.standard_id()
            .xor(counter_id("reversible", self.reversible()))
            .xor(counter_id("round", self.round().get()))
            .xor(variant_id(&STANDARD, V::VARIANT))
    }

    // our 128-bit replacement of the classical Polyglot hash
    pub const fn standard_id(self) -> Id {
        self.transposition_id(&STANDARD)
    }

    // the classical Polyglot hash, just embedded into u128
    pub const fn polyglot_id(self) -> Id {
        self.transposition_id(&POLYGLOT)
    }

    pub const fn transposition_id(self, basis: &Basis) -> Id {
        // In validated positions, en_passant is normalized to effective rights.
        self.apparent_id(basis)
            .xor(castle_id(basis, self.castles()))
            .xor(en_passant_id(basis, self.en_passant()))
    }
}

impl<V: Variant> Position<V> {
    // What one typically sees in a depicted position: The board, and the player to move
    pub const fn apparent_id(self, basis: &Basis) -> Id {
        self.board().id(basis).xor(turn_id(basis, self.turn()))
    }
}

impl Position<Unvalidated> {
    /// For unvalidated positions, there is no "only effective e.p"
    /// invariant, so we normalize it.
    pub const fn normalized_transposition_id(self, basis: &Basis) -> Id {
        self.apparent_id(basis)
            .xor(castle_id(basis, self.castles()))
            .xor(en_passant_id(basis, self.effective_en_passant()))
    }
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

        finite_for!(player in Player {
            finite_for!(role in Role {
                let mut squares = self.players.get(player).intersection_const(self.roles.get(role));
                while let Some(square) = squares.pop_first() {
                    id = id.xor(basis.board.get(square).get(player).get(role));
                }
            });
        });

        id
    }
}

fn counter_id(prefix: &str, counter: u32) -> Id {
    hash(format!("{prefix}:{counter}").as_bytes())
}

const fn turn_id(basis: &Basis, turn: Player) -> Id {
    basis.turn.get(turn)
}

const fn variant_id(basis: &Basis, variant: VariantEnum) -> Id {
    basis.variant.get(variant)
}

const fn en_passant_id(basis: &Basis, en_passant: Option<EnPassant>) -> Id {
    match en_passant {
        Some(square) => basis.en_passant.get(square),
        None => Id(0),
    }
}

const fn castle_id(basis: &Basis, castles: Castles) -> Id {
    let mut id = Id(0);

    finite_for!(player in Player {
        finite_for!(side in Side {
            if let Some(file) = castles.get(player, side) {
                id = id.xor(basis.castle.get(player).get(file));
            }
        });
    });

    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Move,
        game::Cursor,
        position::{Role::*, Square::*},
        variant::Chess,
    };

    const COLLISION_FEN: &str = "2b1kqr1/p2p3p/3p4/p2PpP2/PpP2p2/6P1/8/RRB1KQ1N w - - 21 32";
    const COLLISION_GAME: &str = "
        1. e4 c5 2. Ba6 bxa6 3. b4 g6 4. f4 Bh6 5. f5 Bf4 6. g3 Nf6
        7. gxf4 Nh5 8. c4 cxb4 9. Ne2 Ng3 10. Nd4 a5 11. hxg3 Nc6
        12. Nxc6 Rb8 13. Nxb8 g5 14. d4 f6 15. Na6 gxf4 16. Nc5 Rg8
        17. Nb7 Rh8 18. Nd6+ exd6 19. e5 fxe5 20. d5 Qf6 21. a4 Rg8
        22. Nd2 Qf7 23. Rb1 Qf6 24. Ne4 Qf7 25. Rh2 Qf6 26. Nf2 Qf7
        27. Nh1 Qf6 28. Qd3 Qf7 29. Qf1 Qf6 30. Ra2 Qf7 31. Raa1 Qf8 *";
    const START: Position<Chess> = Position::start();

    #[test]
    fn documents_game_ids() {
        use crate::formats::{Parser as _, pgn};

        // start position, no moves
        let game = Game::new(START);
        let id = game.id();
        assert_eq!(id.to_string(), "LJFnWri3B3piKdWLUSBGWo");
        assert_eq!(id.u128(), 207725053367679635200992249076853996316);

        // 1. e4 on start position
        let mut cursor = Cursor::new(game);
        cursor.push(Move::normal(Pawn, E2, E4)).unwrap();
        let e4 = cursor.into_inner();
        assert_eq!(e4.id().to_string(), "4mU2d1B9K3Not73feSJdFo");
        assert_eq!(e4.id().u128(), 40545599516303226419867082063332202050);

        // The weird game
        let pgn = pgn::game.parse(COLLISION_GAME).unwrap();
        let game: Game = pgn.try_into().unwrap();
        assert_eq!(game.id().to_string(), "Pwh8p1hKRz1HWnZGm444GT");
        assert_eq!(game.id().u128(), 246966137601676322552589085536778212564);
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
        let start = Position::start();
        let position = Chess::from_fen(COLLISION_FEN).unwrap();

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

        let expected = Chess::from_fen(COLLISION_FEN).unwrap();

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
            Chess::from_fen("2b1k3/4p3/3p1p2/p2P2p1/P2P4/2P2PP1/4P3/2NQKB2 b - - 0 1").unwrap();

        assert_eq!(position.polyglot_id(), Id(0));
        assert_ne!(position.transposition_id(&STANDARD), Id(0));
    }
}
