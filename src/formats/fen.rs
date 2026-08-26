use core::num::NonZeroU32;

#[cfg(test)]
use crate::variant::Chess;
use crate::{
    bitboard::Bitboard,
    position::{
        Board, File, Player, Players, Position, Rank, Role, Roles, Side, Sides, Square, Variant,
        en_passant,
    },
    variant::Unvalidated,
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

impl<V: Variant> Position<V> {
    pub fn from_fen(fen: &str) -> Result<Self> {
        let position = position_fen.parse(fen).map_err(|_| Error::Invalid(fen.to_string()))?;
        Position::new(position).map_err(|_| Error::Invalid(fen.to_string()))
    }
}

impl<V> Position<V> {
    pub fn fen(&self) -> String {
        format!(
            "{} {} {} {} {} {}",
            self.board.fen(),
            self.turn.fen(),
            self.castle.fen(),
            en_passant_square(self.en_passant),
            self.reversible,
            self.round
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

impl Players<Sides> {
    pub fn fen(self) -> String {
        use Player::*;
        use Side::*;

        let mut fen = String::new();
        if self[White][King] {
            fen.push('K');
        }
        if self[White][Queen] {
            fen.push('Q');
        }
        if self[Black][King] {
            fen.push('k');
        }
        if self[Black][Queen] {
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

fn castle(input: &mut Input<'_>) -> ModalResult<Players<Sides>> {
    use Player::*;
    use Side::*;

    alt((
        '-'.value(Default::default()),
        repeat(1..=4, one_of(|c| "KQkq".contains(c))).map(|letters: Vec<char>| {
            let mut castle: Players<Sides> = Default::default();
            for letter in letters {
                match letter {
                    'K' => castle[White][King] = true,
                    'Q' => castle[White][Queen] = true,
                    'k' => castle[Black][King] = true,
                    'q' => castle[Black][Queen] = true,
                    _ => unreachable!(),
                }
            }
            castle
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

fn rights_fen(
    input: &mut Input<'_>,
) -> ModalResult<(Player, Players<Sides>, Option<en_passant::Square>)> {
    (player, preceded(space0, castle), preceded(space0, en_passant)).parse_next(input)
}

fn counters_fen(input: &mut Input<'_>) -> ModalResult<(u32, NonZeroU32)> {
    (dec_uint, preceded(space1, dec_uint.verify_map(NonZeroU32::new))).parse_next(input)
}

pub fn position_fen(input: &mut Input<'_>) -> ModalResult<Position<Unvalidated>> {
    let board = board_fen.parse_next(input)?;
    let (turn, castle, en_passant) = preceded(space0, rights_fen).parse_next(input)?;
    let (reversible, round) = preceded(space0, counters_fen).parse_next(input)?;
    Ok(Position { board, turn, castle, en_passant, reversible, round, variant: Unvalidated })
}

const fn missing_counters() -> (u32, NonZeroU32) {
    (0, NonZeroU32::MIN)
}

#[allow(clippy::type_complexity)]
fn missing_non_board() -> ((Player, Players<Sides>, Option<en_passant::Square>), (u32, NonZeroU32))
{
    ((Player::White, Default::default(), None), missing_counters())
}

/// Parse a position, allowing missing trailing counters, or only position
pub fn position_partial_fen(input: &mut Input<'_>) -> ModalResult<Position<Unvalidated>> {
    let board = board_fen.parse_next(input)?;
    let ((turn, castle, en_passant), (reversible, round)) = opt((
        preceded(space0, rights_fen),
        opt(preceded(space0, counters_fen))
            .map(|counters| counters.unwrap_or_else(missing_counters)),
    ))
    .map(|rest| rest.unwrap_or_else(missing_non_board))
    .parse_next(input)?;
    Ok(Position { board, turn, castle, en_passant, reversible, round, variant: Unvalidated })
}

#[test]
fn board_fen_example() {
    use Player::*;
    use Side::*;

    println!("{:?}", board_fen.parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR").unwrap());
    println!(
        "{:?}",
        board_fen.parse_next(&mut "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNRxxx").unwrap()
    );

    let fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3 1 3";
    let position = position_fen.parse(fen).unwrap();
    assert_eq!(position.turn, Black);
    assert!(position.castle[Black][King]);
    assert!(position.castle[Black][Queen]);
    assert!(position.castle[White][King]);
    assert!(position.castle[White][Queen]);
    assert_eq!(position.en_passant.map(Into::into), Some(Square::new(File::E, Rank::Three)));
    assert_eq!(position.reversible, 1);
    assert_eq!(u32::from(position.round), 3);
    assert_eq!(Position::<Chess>::new(position).unwrap().fen(), fen);
    assert_eq!(Position::<Chess>::from_fen(fen).unwrap().fen(), fen);

    let partial_fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3";
    let position = position_partial_fen.parse(partial_fen).unwrap();
    assert_eq!(position.turn, Black);
    assert!(position.castle[Black][King]);
    assert!(position.castle[Black][Queen]);
    assert!(position.castle[White][King]);
    assert!(position.castle[White][Queen]);
    assert_eq!(position.en_passant.map(Into::into), Some(Square::new(File::E, Rank::Three)));
    assert_eq!(position.reversible, 0);
    assert_eq!(u32::from(position.round), 1);
    assert_eq!(
        Position::<Chess>::new(position).unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );

    let position =
        position_partial_fen.parse("rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR").unwrap();
    assert_eq!(position.turn, White);
    assert!(!position.castle[Black][King]);
    assert!(!position.castle[Black][Queen]);
    assert!(!position.castle[White][King]);
    assert!(!position.castle[White][Queen]);
    assert_eq!(position.en_passant, None);
    assert_eq!(position.reversible, 0);
    assert_eq!(u32::from(position.round), 1);
    assert_eq!(
        Position::<Chess>::new(position).unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR w - - 0 1"
    );
}
