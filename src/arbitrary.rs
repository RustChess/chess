use arbitrary::{Arbitrary, Error, Result, Unstructured};

use crate::{Game, Id, Position, formats::Pgn, id::base58::Base58};

impl<'a> Arbitrary<'a> for Id {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        u128::arbitrary(input).map(Self)
    }
}

impl<'a> Arbitrary<'a> for Base58 {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        Vec::<u8>::arbitrary(input).map(|bytes| Self::new(&bytes))
    }
}

impl<'a> Arbitrary<'a> for Game {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        let choices = Vec::<u8>::arbitrary(input)?;
        let mut cursor = Game::new(Position::start()).into_cursor();

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

impl<'a> Arbitrary<'a> for Pgn {
    fn arbitrary(input: &mut Unstructured<'a>) -> Result<Self> {
        Game::arbitrary(input).map(Into::into)
    }
}
