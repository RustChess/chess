//! Standard Algebraic Notation.

use core::fmt;

use crate::position::{File, Rank, Role, Side, Square};

use super::{StrInput as Input, prelude::*};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct San {
    pub play: Move,
    pub check: Option<Check>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Move {
    Normal {
        role: Role,
        file: Option<File>,
        rank: Option<Rank>,
        capture: bool,
        to: Square,
        promotion: Option<Role>,
    },
    Castle(Side),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Check {
    Check,
    Checkmate,
}

impl fmt::Display for San {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.play.fmt(f)?;
        if let Some(check) = self.check {
            check.fmt(f)?;
        }
        Ok(())
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Move::Normal { role, file, rank, capture, to, promotion } => {
                if role != Role::Pawn {
                    role.upper().fmt(f)?;
                }
                if let Some(file) = file {
                    file.fmt(f)?;
                }
                if let Some(rank) = rank {
                    rank.fmt(f)?;
                }
                if capture {
                    write!(f, "x")?;
                }
                to.fmt(f)?;
                if let Some(promotion) = promotion {
                    write!(f, "={}", promotion.upper())?;
                }
                Ok(())
            }
            Move::Castle(Side::King) => write!(f, "O-O"),
            Move::Castle(Side::Queen) => write!(f, "O-O-O"),
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Check::Check => write!(f, "+"),
            Check::Checkmate => write!(f, "#"),
        }
    }
}

pub fn san(input: &mut Input<'_>) -> ModalResult<San> {
    trim(input);
    let play = alt((castle, normal)).context(StrContext::Label("SAN move")).parse_next(input)?;
    let check = opt(check).context(StrContext::Label("SAN check")).parse_next(input)?;
    trim(input);
    Ok(San { play, check })
}

fn castle(input: &mut Input<'_>) -> ModalResult<Move> {
    alt((
        "O-O-O".value(Move::Castle(Side::Queen)),
        "0-0-0".value(Move::Castle(Side::Queen)),
        "O-O".value(Move::Castle(Side::King)),
        "0-0".value(Move::Castle(Side::King)),
    ))
    .context(StrContext::Label("SAN castle"))
    .parse_next(input)
}

fn normal(input: &mut Input<'_>) -> ModalResult<Move> {
    normal_inner.context(StrContext::Label("SAN normal")).parse_next(input)
}

fn normal_inner(input: &mut Input<'_>) -> ModalResult<Move> {
    let role = opt(role).parse_next(input)?.unwrap_or(Role::Pawn);
    let mut explicit_file = None;
    let mut explicit_rank = None;

    let first = opt(file).parse_next(input)?;
    let second = opt(rank).parse_next(input)?;

    if opt('x').parse_next(input)?.is_some() {
        if role == Role::Pawn {
            explicit_file = first;
        } else {
            explicit_file = first;
            explicit_rank = second;
        }
        let to = square(input)?;
        let promotion = opt(promotion).parse_next(input)?;
        return Ok(Move::Normal {
            role,
            file: explicit_file,
            rank: explicit_rank,
            capture: true,
            to,
            promotion,
        });
    }

    match (first, second) {
        (Some(to_file), Some(to_rank)) => {
            let to = Square::new(to_file, to_rank);
            let promotion = opt(promotion).parse_next(input)?;
            Ok(Move::Normal {
                role,
                file: explicit_file,
                rank: explicit_rank,
                capture: false,
                to,
                promotion,
            })
        }
        (Some(file), None) => {
            explicit_file = Some(file);
            let to = square(input)?;
            Ok(Move::Normal {
                role,
                file: explicit_file,
                rank: explicit_rank,
                capture: false,
                to,
                promotion: None,
            })
        }
        (None, Some(rank)) => {
            explicit_rank = Some(rank);
            let to = square(input)?;
            Ok(Move::Normal {
                role,
                file: explicit_file,
                rank: explicit_rank,
                capture: false,
                to,
                promotion: None,
            })
        }
        (None, None) => err(),
    }
}

fn trim(input: &mut Input<'_>) {
    *input = input.trim_start();
}

fn square(input: &mut Input<'_>) -> ModalResult<Square> {
    (file, rank).map(|(file, rank)| Square::new(file, rank)).parse_next(input)
}

fn role(input: &mut Input<'_>) -> ModalResult<Role> {
    one_of(|c| "NBRQK".contains(c)).map(Role::panicky_from_char).parse_next(input)
}

fn promotion_role(input: &mut Input<'_>) -> ModalResult<Role> {
    one_of(|c| "NBRQ".contains(c)).map(Role::panicky_from_char).parse_next(input)
}

fn promotion(input: &mut Input<'_>) -> ModalResult<Role> {
    opt('=').parse_next(input)?;
    promotion_role(input)
}

fn file(input: &mut Input<'_>) -> ModalResult<File> {
    one_of(|c| "abcdefgh".contains(c)).map(File::panicky_from_char).parse_next(input)
}

fn rank(input: &mut Input<'_>) -> ModalResult<Rank> {
    one_of(|c| "12345678".contains(c)).map(|c| Rank::panicky_new(c as u8 - b'1')).parse_next(input)
}

fn check(input: &mut Input<'_>) -> ModalResult<Check> {
    alt(('+'.value(Check::Check), '#'.value(Check::Checkmate))).parse_next(input)
}

fn err<T>() -> ModalResult<T> {
    use winnow::error::{ContextError, ErrMode};

    Err(ErrMode::Backtrack(ContextError::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pawn_move() {
        let parsed = san.parse("e4").unwrap();
        assert_eq!(
            parsed.play,
            Move::Normal {
                role: Role::Pawn,
                file: None,
                rank: None,
                capture: false,
                to: Square::E4,
                promotion: None,
            }
        );
    }

    #[test]
    fn tolerates_whitespace() {
        let parsed = san.parse("  O-O-O#  ").unwrap();
        assert_eq!(parsed.play, Move::Castle(Side::Queen));
        assert_eq!(parsed.check, Some(Check::Checkmate));
    }

    #[test]
    fn parses_piece_move() {
        let parsed = san.parse("Nbd2+").unwrap();
        assert_eq!(
            parsed.play,
            Move::Normal {
                role: Role::Knight,
                file: Some(File::B),
                rank: None,
                capture: false,
                to: Square::D2,
                promotion: None,
            }
        );
        assert_eq!(parsed.check, Some(Check::Check));
    }

    #[test]
    fn parses_capture_promotion_and_castle() {
        let parsed = san.parse("exd8=Q#").unwrap();
        assert_eq!(
            parsed.play,
            Move::Normal {
                role: Role::Pawn,
                file: Some(File::E),
                rank: None,
                capture: true,
                to: Square::D8,
                promotion: Some(Role::Queen),
            }
        );
        assert_eq!(parsed.check, Some(Check::Checkmate));

        let parsed = san.parse("O-O").unwrap();
        assert_eq!(parsed.play, Move::Castle(Side::King));

        let parsed = san.parse("0-0-0").unwrap();
        assert_eq!(parsed.play, Move::Castle(Side::Queen));
    }

    #[test]
    fn displays_san() {
        assert_eq!(san.parse("Nbd2+").unwrap().to_string(), "Nbd2+");
        assert_eq!(san.parse("exd8=Q#").unwrap().to_string(), "exd8=Q#");
        assert_eq!(san.parse("O-O-O").unwrap().to_string(), "O-O-O");
        assert_eq!(san.parse("0-0").unwrap().to_string(), "O-O");
        assert_eq!(san.parse("exd1Q+").unwrap().to_string(), "exd1=Q+");
    }
}
