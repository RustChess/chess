use crate::position::{PlayerTable, RoleTable, SideTable, en_passant, square::SquareTable};

use super::{Id, fold, hash};

#[cfg(feature = "empty-id")]
pub const STANDARD: Basis = Basis::empty();
#[cfg(all(not(feature = "empty-id"), feature = "const-fn-standard-id"))]
// This allow turns the error into a warning, which cannot currently be suppressed.
#[allow(long_running_const_eval)]
pub const STANDARD: Basis = Basis::generate_standard_impl();
#[cfg(all(not(feature = "empty-id"), not(feature = "const-fn-standard-id")))]
include!("standard.rs");

#[cfg(feature = "empty-id")]
pub const POLYGLOT: Basis = Basis::empty();
#[cfg(not(feature = "empty-id"))]
include!("polyglot.rs");

#[derive(Clone, Copy, Debug)]
pub struct Basis {
    pub board: SquareTable<PlayerTable<RoleTable<Id>>>,
    pub turn: PlayerTable<Id>,
    pub castle: PlayerTable<SideTable<Id>>,
    pub en_passant: EnPassant,
    pub variant: Variant,
}

#[derive(Clone, Copy, Debug)]
pub struct EnPassant {
    pub none: Id,
    pub square: en_passant::SquareTable<Id>,
}

#[derive(Clone, Copy, Debug)]
pub struct Variant {
    pub chess: Id,
    pub freestyle: Id,
}

impl Basis {
    pub const fn empty() -> Self {
        Self {
            board: SquareTable::empty(),
            turn: PlayerTable::empty(),
            castle: PlayerTable::empty(),
            en_passant: EnPassant::empty(),
            variant: Variant::empty(),
        }
    }

    const fn generate_standard_impl() -> Self {
        Self {
            board: board_map(),
            turn: player_map(b"turn:"),
            castle: castle_map(),
            en_passant: EnPassant::generate(),
            variant: Variant::generate(),
        }
    }

    #[cfg(feature = "empty-id")]
    pub const fn generate_standard() -> Self {
        Self::generate_standard_impl()
    }
}

impl Default for Basis {
    fn default() -> Self {
        Self::generate_standard_impl()
    }
}

impl EnPassant {
    const fn empty() -> Self {
        Self { none: Id(0), square: en_passant::SquareTable::empty() }
    }

    const fn generate() -> Self {
        Self { none: hash(b"en-passant:none"), square: en_passant_square_map(b"en-passant:") }
    }
}

impl Variant {
    const fn empty() -> Self {
        Self { chess: Id(0), freestyle: Id(0) }
    }

    const fn generate() -> Self {
        Self { chess: hash(b"variant:chess"), freestyle: hash(b"variant:freestyle") }
    }
}

const fn board_map() -> SquareTable<PlayerTable<RoleTable<Id>>> {
    let mut squares = SquareTable::empty();
    let mut square = 0;
    while square < 64 {
        let mut players = PlayerTable::empty();
        let mut player = 0;
        while player < 2 {
            let mut roles = RoleTable::empty();
            let mut role = 0;
            while role < 6 {
                roles.all[role] = hash_7(
                    b"board:",
                    file(square),
                    rank(square),
                    b":",
                    player_name(player),
                    b":",
                    role_name(role),
                );
                role += 1;
            }
            players.all[player] = roles;
            player += 1;
        }
        squares.all[square] = players;
        square += 1;
    }
    squares
}

const fn castle_map() -> PlayerTable<SideTable<Id>> {
    let mut players = PlayerTable::empty();
    let mut player = 0;
    while player < 2 {
        let mut sides = SideTable::empty();
        let mut side = 0;
        while side < 2 {
            sides.all[side] = hash_4(b"castle:", player_name(player), b":", side_name(side));
            side += 1;
        }
        players.all[player] = sides;
        player += 1;
    }
    players
}

const fn player_map(prefix: &[u8]) -> PlayerTable<Id> {
    PlayerTable::new([hash_2(prefix, b"black"), hash_2(prefix, b"white")])
}

const fn en_passant_square_map(prefix: &[u8]) -> en_passant::SquareTable<Id> {
    let mut squares = en_passant::SquareTable::empty();
    let mut square = 0;
    while square < en_passant::Square::ALL.len() {
        let en_passant_square = en_passant::Square::ALL[square];
        let board_square = en_passant_square.square() as usize;
        squares.all[square] = hash_3(prefix, file(board_square), rank(board_square));
        square += 1;
    }
    squares
}

const fn file(square: usize) -> &'static [u8] {
    match square % 8 {
        0 => b"a",
        1 => b"b",
        2 => b"c",
        3 => b"d",
        4 => b"e",
        5 => b"f",
        6 => b"g",
        _ => b"h",
    }
}

const fn rank(square: usize) -> &'static [u8] {
    match square / 8 {
        0 => b"1",
        1 => b"2",
        2 => b"3",
        3 => b"4",
        4 => b"5",
        5 => b"6",
        6 => b"7",
        _ => b"8",
    }
}

const fn player_name(player: usize) -> &'static [u8] {
    match player {
        0 => b"black",
        _ => b"white",
    }
}

const fn role_name(role: usize) -> &'static [u8] {
    match role {
        0 => b"pawn",
        1 => b"knight",
        2 => b"bishop",
        3 => b"rook",
        4 => b"queen",
        _ => b"king",
    }
}

const fn side_name(side: usize) -> &'static [u8] {
    match side {
        0 => b"king",
        _ => b"queen",
    }
}

const fn hash_2(a: &[u8], b: &[u8]) -> Id {
    Id(fold(sha2_const::Sha256::new().update(a).update(b).finalize()))
}

const fn hash_3(a: &[u8], b: &[u8], c: &[u8]) -> Id {
    Id(fold(sha2_const::Sha256::new().update(a).update(b).update(c).finalize()))
}

const fn hash_4(a: &[u8], b: &[u8], c: &[u8], d: &[u8]) -> Id {
    Id(fold(sha2_const::Sha256::new().update(a).update(b).update(c).update(d).finalize()))
}

const fn hash_7(a: &[u8], b: &[u8], c: &[u8], d: &[u8], e: &[u8], f: &[u8], g: &[u8]) -> Id {
    Id(fold(
        sha2_const::Sha256::new()
            .update(a)
            .update(b)
            .update(c)
            .update(d)
            .update(e)
            .update(f)
            .update(g)
            .finalize(),
    ))
}
