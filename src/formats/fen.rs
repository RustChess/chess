use core::{marker::PhantomData, num::NonZeroU32};

#[cfg(test)]
use crate::variant::Chess;
use crate::{
    bitboard::Bitboard,
    position::{
        Board, Castles, File, Player, Players, Position, Rank, Role, Roles, Side, Square,
        en_passant,
    },
    variant::{Unvalidated, Validate, Variant},
};

use super::{StrInput as Input, prelude::*};

// There's a choice to be made whether to require in-between whitespace or not,
// we accept "compact" FEN without it. The "board" parser finishes once the
// 64 squares are filled, so it won't "swallow" the turn parser's input.

// pub struct Fen(String);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid FEN: {0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

// Lenient - fills up with default values.
pub fn position_unvalidated(input: &mut Input<'_>) -> ModalResult<Position<Unvalidated>> {
    let board = board_fen.parse_next(input)?;
    let Fields { turn, castles, en_passant, reversible, round } = fields_fen.parse_next(input)?;
    Ok(Position { board, turn, castles, en_passant, reversible, round, variant: PhantomData })
}

impl Unvalidated {
    pub fn from_fen(fen: &str) -> Result<Position<Unvalidated>> {
        position_unvalidated.parse(fen).map_err(|_| Error::Invalid(fen.to_string()))
    }
}

impl<V: Validate> Position<V> {
    pub fn from_fen(fen: &str) -> Result<Self> {
        let position = Unvalidated::from_fen(fen)?;
        position.validate().map_err(|_| Error::Invalid(fen.to_string()))
    }
}

impl<V> Position<V> {
    pub fn apparent_fen(&self) -> String {
        format!("{} {}", self.board.fen(), self.turn.fen(),)
    }

    pub fn fen(&self) -> String {
        format!(
            "{} {} {} {} {} {}",
            self.board.fen(),
            self.turn.fen(),
            self.castles.fen(),
            en_passant_square(self.en_passant),
            self.reversible,
            self.round
        )
    }
}

impl<V: Variant> Position<V> {
    pub fn transposition_fen(&self) -> String {
        format!(
            "{} {} {} {}",
            self.board.fen(),
            self.turn.fen(),
            self.castles.fen(),
            en_passant_square(self.effective_en_passant()),
        )
    }
}

impl Board {
    pub fn fen(self) -> String {
        let mut fen = String::new();

        for rank in Rank::iter_rev() {
            if rank != Rank::Eight {
                fen.push('/');
            }

            let mut empty = 0;
            for file in File::iter() {
                let square = Square::new(file, rank);
                if let Some(piece) = self.piece_at(square) {
                    if empty > 0 {
                        fen.push(char::from_digit(empty, 10).unwrap());
                        empty = 0;
                    }
                    fen.push(piece.char());
                } else {
                    empty += 1;
                }
            }
            if empty > 0 {
                fen.push(char::from_digit(empty, 10).unwrap());
            }
        }

        fen
    }
}

impl Player {
    pub fn fen(self) -> char {
        match self {
            Player::Black => 'b',
            Player::White => 'w',
        }
    }
}

impl Castles {
    fn fen(self) -> String {
        use Player::*;
        use Side::*;

        let mut fen = String::new();
        if self.has(White, King) {
            fen.push('K');
        }
        if self.has(White, Queen) {
            fen.push('Q');
        }
        if self.has(Black, King) {
            fen.push('k');
        }
        if self.has(Black, Queen) {
            fen.push('q');
        }
        if fen.is_empty() {
            fen.push('-');
        }
        fen
    }
}

fn en_passant_square(en_passant: Option<en_passant::Square>) -> String {
    en_passant.map_or_else(|| "-".to_string(), |square| Square::from(square).to_string())
}

fn is_board_fen_char(c: char) -> bool {
    "12345678pnbrkqPNBRKQ".contains(c)
}

fn board_fen_char(input: &mut Input<'_>) -> ModalResult<char> {
    one_of(is_board_fen_char).parse_next(input)
}

pub fn board_fen(input: &mut Input<'_>) -> ModalResult<Board> {
    // trim leading whitespace
    *input = input.trim_start();

    let mut players: Players<Bitboard> = Default::default();
    let mut roles: Roles<Bitboard> = Default::default();
    let mut it = Square::rank_rev_iter();
    while let Some(square) = it.next() {
        match preceded(opt('/'), board_fen_char).parse_next(input)? {
            i @ '1'..='8' => {
                for _ in '1'..i {
                    if it.next().is_none() {
                        // TODO: error out here instead
                        break;
                    }
                }
            }
            piece => {
                let square = Bitboard::from(square);
                if piece.is_lowercase() {
                    players.black |= square;
                } else {
                    players.white |= square;
                };
                let role = Role::panicky_from_char(piece);
                roles[role] |= square;
            }
        }
    }
    Ok(Board { occupied: players.black | players.white, players, roles })
}

fn player(input: &mut Input<'_>) -> ModalResult<Player> {
    one_of(|c| "bw".contains(c))
        .map(|c| match c {
            'b' => Player::Black,
            'w' => Player::White,
            _ => unreachable!(),
        })
        .parse_next(input)
}

fn castle(input: &mut Input<'_>) -> ModalResult<Castles> {
    use Player::*;
    use Side::*;

    alt((
        '-'.value(Castles::empty()),
        repeat(1..=4, one_of(|c| "KQkq".contains(c))).map(|letters: Vec<char>| {
            let mut castles = Castles::empty();
            for letter in letters {
                match letter {
                    'K' => castles.set(White, King, King.chess_rook()),
                    'Q' => castles.set(White, Queen, Queen.chess_rook()),
                    'k' => castles.set(Black, King, King.chess_rook()),
                    'q' => castles.set(Black, Queen, Queen.chess_rook()),
                    _ => unreachable!(),
                }
            }
            castles
        }),
    ))
    .parse_next(input)
}

fn file(input: &mut Input<'_>) -> ModalResult<File> {
    one_of(|c| "abcdefgh".contains(c)).map(File::panicky_from_char).parse_next(input)
}

fn en_passant(input: &mut Input<'_>) -> ModalResult<Option<en_passant::Square>> {
    alt((
        '-'.value(None),
        terminated(file, '3').map(|file| Some(Square::new(file, Rank::Three).try_into().unwrap())),
        terminated(file, '6').map(|file| Some(Square::new(file, Rank::Six).try_into().unwrap())),
    ))
    .parse_next(input)
}

fn reversible(input: &mut Input<'_>) -> ModalResult<u32> {
    dec_uint.parse_next(input)
}

fn round(input: &mut Input<'_>) -> ModalResult<NonZeroU32> {
    dec_uint.verify_map(NonZeroU32::new).parse_next(input)
}

struct Fields {
    turn: Player,
    castles: Castles,
    en_passant: Option<en_passant::Square>,
    reversible: u32,
    round: NonZeroU32,
}

fn fields_fen(input: &mut Input<'_>) -> ModalResult<Fields> {
    let Some(turn) = opt(preceded(space0, player)).parse_next(input)? else {
        return Ok(default_fields());
    };

    let Some(castles) = opt(preceded(space0, castle)).parse_next(input)? else {
        return Ok(Fields { turn, ..default_fields() });
    };

    let Some(en_passant) = opt(preceded(space0, en_passant)).parse_next(input)? else {
        return Ok(Fields { turn, castles, ..default_fields() });
    };

    let Some(reversible) = opt(preceded(space0, reversible)).parse_next(input)? else {
        return Ok(Fields { turn, castles, en_passant, ..default_fields() });
    };

    let Some(round) = opt(preceded(space0, round)).parse_next(input)? else {
        return Ok(Fields { turn, castles, en_passant, reversible, ..default_fields() });
    };

    Ok(Fields { turn, castles, en_passant, reversible, round })
}

fn default_fields() -> Fields {
    Fields {
        turn: Player::White,
        castles: Castles::empty(),
        en_passant: None,
        reversible: 0,
        round: NonZeroU32::MIN,
    }
}

#[test]
fn board_fen_example() {
    use File::*;
    use Player::*;
    use Rank::*;
    use Side::*;

    println!("{:?}", board_fen.parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR").unwrap());
    println!(
        "{:?}",
        board_fen.parse_next(&mut "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNRxxx").unwrap()
    );

    let fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3 1 3";
    let position = position_unvalidated.parse(fen).unwrap();
    assert_eq!(position.turn, Black);
    assert!(position.castles.has(Black, King));
    assert!(position.castles.has(Black, Queen));
    assert!(position.castles.has(White, King));
    assert!(position.castles.has(White, Queen));
    assert_eq!(position.en_passant.map(Into::into), Some(Square::new(E, Three)));
    assert_eq!(position.reversible, 1);
    assert_eq!(u32::from(position.round), 3);
    assert_eq!(position.validate::<Chess>().unwrap().fen(), fen);
    assert_eq!(Position::<Chess>::from_fen(fen).unwrap().fen(), fen);

    let partial_fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3";
    let position = position_unvalidated.parse(partial_fen).unwrap();
    assert_eq!(position.turn, Black);
    assert!(position.castles.has(Black, King));
    assert!(position.castles.has(Black, Queen));
    assert!(position.castles.has(White, King));
    assert!(position.castles.has(White, Queen));
    assert_eq!(position.en_passant.map(Into::into), Some(Square::new(E, Three)));
    assert_eq!(position.reversible, 0);
    assert_eq!(u32::from(position.round), 1);
    assert_eq!(
        position.validate::<Chess>().unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );
    assert_eq!(
        Position::<Chess>::from_fen(partial_fen).unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );

    let board_fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR";
    let position = position_unvalidated.parse(board_fen).unwrap();
    assert_eq!(position.turn, White);
    assert!(!position.castles.has(Black, King));
    assert!(!position.castles.has(Black, Queen));
    assert!(!position.castles.has(White, King));
    assert!(!position.castles.has(White, Queen));
    assert_eq!(position.en_passant, None);
    assert_eq!(position.reversible, 0);
    assert_eq!(u32::from(position.round), 1);
    assert_eq!(
        position.validate::<Chess>().unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR w - - 0 1"
    );
    assert_eq!(
        Position::<Chess>::from_fen(board_fen).unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR w - - 0 1"
    );
}
