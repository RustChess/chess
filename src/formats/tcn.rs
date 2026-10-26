//! Chess.com compact move notation.

use core::{fmt, ops::Deref, str::FromStr};

use crate::{Role, Square, formats::uci};

use super::str_prelude::*;

const ALPHABET: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!?{~}(^)[_]@#$,./&-*++=";

#[derive(Clone, Debug, DeserializeFromStr, Eq, PartialEq, SerializeDisplay)]
pub struct Tcn(pub Vec<Move>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Move {
    Play(uci::Move),
    Drop { role: Role, to: Square },
}

#[derive(Debug, thiserror::Error)]
#[error("invalid TCN")]
pub struct Error;

impl Deref for Tcn {
    type Target = Vec<Move>;

    fn deref(&self) -> &Vec<Move> {
        &self.0
    }
}

impl Tcn {
    pub fn uci(&self) -> impl Iterator<Item = uci::Move> + '_ {
        self.iter().filter_map(|play| match play {
            Move::Play(play) => Some(*play),
            Move::Drop { .. } => None,
        })
    }
}

impl fmt::Display for Tcn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for play in self.iter() {
            play.fmt(f)?;
        }
        Ok(())
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (a, b) = match *self {
            Move::Play(play) => encode_play(play).ok_or(fmt::Error)?,
            Move::Drop { role, to } => (encode_drop(role).ok_or(fmt::Error)?, to.index()),
        };
        write!(f, "{}{}", character(a).ok_or(fmt::Error)?, character(b).ok_or(fmt::Error)?)
    }
}

impl FromStr for Tcn {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        tcn.parse(text).map_err(|_| Error)
    }
}

pub fn tcn(input: &mut Input<'_>) -> ModalResult<Tcn> {
    repeat(0.., play).map(Tcn).parse_next(input)
}

pub fn play(input: &mut Input<'_>) -> ModalResult<Move> {
    (code, code).verify_map(|(a, b)| decode(a, b)).parse_next(input)
}

fn code(input: &mut Input<'_>) -> ModalResult<usize> {
    one_of(|c| ALPHABET.contains(c))
        .map(|c| ALPHABET.find(c).expect("parsed TCN character"))
        .parse_next(input)
}

fn decode(a: usize, b: usize) -> Option<Move> {
    if a < 64 {
        let from = Square::from_index(a.try_into().ok()?)?;
        let (to, promotion) = if b < 64 {
            (Square::from_index(b.try_into().ok()?)?, None)
        } else if b < 76 {
            let promotion = [Role::Queen, Role::Knight, Role::Rook, Role::Bishop][(b - 64) / 3];
            let direction = ((b - 64) % 3) as isize - 1;
            let advance = if a < 16 { -8 } else { 8 };
            let to = usize::try_from(a as isize + advance + direction).ok()?;
            (Square::from_index(to.try_into().ok()?)?, Some(promotion))
        } else {
            return None;
        };
        Some(Move::Play(uci::Move { from, to, promotion }))
    } else {
        let role = match a {
            79 => Role::Queen,
            80 => Role::Knight,
            81 => Role::Rook,
            82 => Role::Bishop,
            84 => Role::Pawn,
            _ => return None,
        };
        let to = Square::from_index(b.try_into().ok()?)?;
        Some(Move::Drop { role, to })
    }
}

fn encode_play(play: uci::Move) -> Option<(usize, usize)> {
    let from = play.from.index();
    let to = match play.promotion {
        None => play.to.index(),
        Some(promotion) => {
            let promotion = match promotion {
                Role::Queen => 0,
                Role::Knight => 1,
                Role::Rook => 2,
                Role::Bishop => 3,
                _ => return None,
            };
            let advance = if from < 16 { -8 } else { 8 };
            let direction = play.to.index() as isize - from as isize - advance;
            if !(-1..=1).contains(&direction) {
                return None;
            }
            64 + promotion * 3 + usize::try_from(direction + 1).ok()?
        }
    };
    Some((from, to))
}

fn encode_drop(role: Role) -> Option<usize> {
    match role {
        Role::Queen => Some(79),
        Role::Knight => Some(80),
        Role::Rook => Some(81),
        Role::Bishop => Some(82),
        Role::Pawn => Some(84),
        Role::King => None,
    }
}

fn character(code: usize) -> Option<char> {
    ALPHABET.as_bytes().get(code).copied().map(char::from)
}

#[cfg(test)]
mod tests {
    use crate::{Role, formats::uci, square::Square::*};

    use super::*;

    #[test]
    fn parse_plays() {
        let parsed: Tcn = "gvYIeg0~".parse().unwrap();

        assert_eq!(
            parsed.uci().collect::<Vec<_>>(),
            vec![
                uci::Move { from: G1, to: F3, promotion: None },
                uci::Move { from: C7, to: C5, promotion: None },
                uci::Move { from: E1, to: G1, promotion: None },
                uci::Move { from: E7, to: E8, promotion: Some(Role::Queen) },
            ]
        );
        assert_eq!(parsed.to_string(), "gvYIeg0~");
    }

    #[test]
    fn preserve_drop_and_filter_uci() {
        let parsed: Tcn = "+Cma".parse().unwrap();

        assert_eq!(parsed[0], Move::Drop { role: Role::Bishop, to: E4 });
        assert_eq!(
            parsed.uci().collect::<Vec<_>>(),
            vec![uci::Move { from: E2, to: A1, promotion: None }]
        );
        assert_eq!(parsed.to_string(), "+Cma");
    }

    #[test]
    fn reject_invalid_codes() {
        assert!("g".parse::<Tcn>().is_err());
        assert!(",a".parse::<Tcn>().is_err());
        assert!("a&".parse::<Tcn>().is_err());
    }
}
