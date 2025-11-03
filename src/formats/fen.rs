use core::num::NonZeroU32;

use crate::{
    bitboard::Bitboard,
    position::{
        Board, File, Player, Players, Position, Rank, Role, Roles, Side, Sides, Square, en_passant,
        variant::Unvalidated,
    },
};

use super::prelude::*;

// There's a choice to be made whether to require in-between whitespace or not,
// we accept "compact" FEN without it. The "board" parser finishes once the
// 64 squares are filled, so it won't "swallow" the turn parser's input.

// pub struct Fen(String);

fn is_board_fen_char(c: char) -> bool {
    "12345678pnbrkqPNBRKQ".contains(c)
}

fn board_fen_char(input: &mut &str) -> ModalResult<char> {
    one_of(is_board_fen_char).parse_next(input)
}

pub fn board_fen(input: &mut &str) -> ModalResult<Board> {
    // trim leading whitespace
    *input = input.trim_start();

    let mut players: Players<Bitboard> = Default::default();
    let mut roles: Roles<Bitboard> = Default::default();
    let mut it = Square::iter();
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

fn player(input: &mut &str) -> ModalResult<Player> {
    one_of(|c| "bw".contains(c))
        .map(|c| match c {
            'b' => Player::Black,
            'w' => Player::White,
            _ => unreachable!(),
        })
        .parse_next(input)
}

fn castle(input: &mut &str) -> ModalResult<Players<Sides>> {
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

fn file(input: &mut &str) -> ModalResult<File> {
    one_of(|c| "abcdefgh".contains(c)).map(File::panicky_from_char).parse_next(input)
}

fn en_passant(input: &mut &str) -> ModalResult<Option<en_passant::Square>> {
    alt((
        '-'.value(None),
        terminated(file, '3').map(|file| Some(Square::new(file, Rank::Three).try_into().unwrap())),
        terminated(file, '6').map(|file| Some(Square::new(file, Rank::Six).try_into().unwrap())),
    ))
    .parse_next(input)
}

fn rights_fen(
    input: &mut &str,
) -> ModalResult<(Player, Players<Sides>, Option<en_passant::Square>)> {
    (player, preceded(space0, castle), preceded(space0, en_passant)).parse_next(input)
}

fn counters_fen(input: &mut &str) -> ModalResult<(u32, NonZeroU32)> {
    (dec_uint, preceded(space1, dec_uint.verify_map(NonZeroU32::new))).parse_next(input)
}

pub fn position_fen(input: &mut &str) -> ModalResult<Position<Unvalidated>> {
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
pub fn position_partial_fen(input: &mut &str) -> ModalResult<Position<Unvalidated>> {
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

    let position = position_fen
        .parse("rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3 1 3")
        .unwrap();
    assert_eq!(position.turn, Black);
    assert!(position.castle[Black][King]);
    assert!(position.castle[Black][Queen]);
    assert!(position.castle[White][King]);
    assert!(position.castle[White][Queen]);
    assert_eq!(position.en_passant.map(Into::into), Some(Square::new(File::E, Rank::Three)));
    assert_eq!(position.reversible, 1);
    assert_eq!(u32::from(position.round), 3);

    let position = position_partial_fen
        .parse("rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3")
        .unwrap();
    assert_eq!(position.turn, Black);
    assert!(position.castle[Black][King]);
    assert!(position.castle[Black][Queen]);
    assert!(position.castle[White][King]);
    assert!(position.castle[White][Queen]);
    assert_eq!(position.en_passant.map(Into::into), Some(Square::new(File::E, Rank::Three)));
    assert_eq!(position.reversible, 0);
    assert_eq!(u32::from(position.round), 1);

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
}
