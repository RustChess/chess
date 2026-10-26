use arbitrary::{Arbitrary, Error, Result, Unstructured};

use crate::{
    Game, Key, Position, Scharnagl,
    formats::{Fen, Pgn},
    game::{Draw, Mode, Outcome, Win},
    key::base58::Base58,
};

impl<'a> Arbitrary<'a> for Base58 {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        Vec::<u8>::arbitrary(input).map(|bytes| Self::new(&bytes))
    }
}

impl<'a> Arbitrary<'a> for Fen {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        Position::arbitrary(input).map(Into::into)
    }
}

impl<'a> Arbitrary<'a> for Game {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        let choices = Vec::<u8>::arbitrary(input)?;
        let game = match Mode::arbitrary(input)? {
            Mode::Chess => Game::new(Position::start()),
            Mode::Freestyle => Game::freestyle(Position::freestyle(Scharnagl::arbitrary(input)?)),
        };
        let mut cursor = game.into_cursor();

        for choice in choices.into_iter().take(256) {
            let position = cursor.position();
            let legal = position.legal();
            if legal.is_empty() {
                break;
            }
            let play = legal[usize::from(choice) % legal.len()];
            cursor.option_mut_or_push(play).map_err(|_| Error::IncorrectFormat)?;
        }

        Ok(cursor.into_game())
    }
}

impl<'a> Arbitrary<'a> for Key {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        u128::arbitrary(input).map(Self)
    }
}

impl<'a> Arbitrary<'a> for Mode {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        bool::arbitrary(input).map(Into::into)
    }
}

impl<'a> Arbitrary<'a> for Outcome {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        Ok(match u8::arbitrary(input)? % 3 {
            0 => Self::White(Option::<Win>::arbitrary(input)?),
            1 => Self::Black(Option::<Win>::arbitrary(input)?),
            _ => Self::Draw(Option::<Draw>::arbitrary(input)?),
        })
    }
}

impl<'a> Arbitrary<'a> for Win {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        Ok(match u8::arbitrary(input)? % 5 {
            0 => Self::Checkmate,
            1 => Self::Resignation,
            2 => Self::FlagFall,
            3 => Self::Forfeit,
            _ => Self::Abandonment,
        })
    }
}

impl<'a> Arbitrary<'a> for Draw {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        Ok(match u8::arbitrary(input)? % 9 {
            0 => Self::Agreement,
            1 => Self::Stalemate,
            2 => Self::DeadPosition,
            3 => Self::ThreefoldRepetition,
            4 => Self::FivefoldRepetition,
            5 => Self::Repetition,
            6 => Self::FiftyMoves,
            7 => Self::SeventyFiveMoves,
            _ => Self::FlagFall,
        })
    }
}

impl<'a> Arbitrary<'a> for Pgn {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        Game::arbitrary(input).map(Into::into)
    }
}

impl<'a> Arbitrary<'a> for Position {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        let game = Game::arbitrary(input)?;
        let mut position = game.start();
        while let Ok(play) = position.main() {
            position = play.position();
        }
        Ok(position.position())
    }
}

impl<'a> Arbitrary<'a> for Scharnagl {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        let index: u16 = input.int_in_range(0..=959)?;
        Ok(Self::new(index).expect("Scharnagl index in range"))
    }
}
