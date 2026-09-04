use crate::{
    board::*,
    finite::Empty as _,
    position::{EnPassant, EnPassantTable},
    square::*,
};

use super::{Id, fold};

#[cfg(feature = "const-fn-standard-id")]
// This allow turns the error into a warning, which cannot currently be suppressed.
#[allow(long_running_const_eval)]
pub const STANDARD: Basis = Basis::generate_standard();
#[cfg(not(feature = "const-fn-standard-id"))]
include!("standard.rs");

#[cfg(feature = "const-fn-polyglot-id")]
pub const POLYGLOT: Basis = Basis::empty();
#[cfg(not(feature = "const-fn-polyglot-id"))]
include!("polyglot.rs");

type BoardTable<T> = SquareTable<PlayerTable<RoleTable<T>>>;
type CastleTable<T> = PlayerTable<FileTable<T>>;

#[derive(Clone, Copy, Debug)]
pub struct Basis {
    pub board: BoardTable<Id>,
    pub turn: PlayerTable<Id>,
    pub castle: CastleTable<Id>,
    pub en_passant: EnPassantTable<Id>,
}

impl Basis {
    pub const fn empty() -> Self {
        Self {
            board: BoardTable::EMPTY,
            turn: PlayerTable::EMPTY,
            castle: CastleTable::EMPTY,
            en_passant: EnPassantTable::EMPTY,
        }
    }

    pub const fn generate_standard() -> Self {
        Self {
            board: board_table(),
            turn: turn_table(),
            castle: castle_table(),
            en_passant: en_passant_table(),
        }
    }
}

const fn board_table() -> BoardTable<Id> {
    let mut squares = SquareTable::empty();

    finite_for!(square in Square {
        let mut players = PlayerTable::empty();
        finite_for!(player in Player {
            let mut roles = RoleTable::empty();
            finite_for!(role in Role {
                let entry = entry_4("board", square.name(), player.name(), role.name());
                roles.set(role, entry);
            });
            players.set(player, roles);
        });
        squares.set(square, players);
    });
    squares
}

const fn castle_table() -> CastleTable<Id> {
    let mut players = PlayerTable::empty();
    finite_for!(player in Player {
        let mut files = FileTable::empty();
        finite_for!(file in File {
            let entry = entry_3("castle", player.name(), file.name());
            files.set(file, entry);
        });
        players.set(player, files);
    });
    players
}

const fn turn_table() -> PlayerTable<Id> {
    let mut players = PlayerTable::empty();
    finite_for!(player in Player {
        let entry = entry_2("turn", player.name());
        players.set(player, entry);
    });
    players
}

const fn en_passant_table() -> EnPassantTable<Id> {
    let mut squares = EnPassantTable::empty();
    finite_for!(en_passant in EnPassant {
        let entry = entry_2("en-passant", en_passant.name());
        squares.set(en_passant, entry);
    });
    squares
}

const fn entry_2(a: &str, b: &str) -> Id {
    Id(fold(
        sha2_const::Sha256::new().update(a.as_bytes()).update(b":").update(b.as_bytes()).finalize(),
    ))
}

const fn entry_3(a: &str, b: &str, c: &str) -> Id {
    Id(fold(
        sha2_const::Sha256::new()
            .update(a.as_bytes())
            .update(b":")
            .update(b.as_bytes())
            .update(b":")
            .update(c.as_bytes())
            .finalize(),
    ))
}

const fn entry_4(a: &str, b: &str, c: &str, d: &str) -> Id {
    Id(fold(
        sha2_const::Sha256::new()
            .update(a.as_bytes())
            .update(b":")
            .update(b.as_bytes())
            .update(b":")
            .update(c.as_bytes())
            .update(b":")
            .update(d.as_bytes())
            .finalize(),
    ))
}
