//! Standard Algebraic Notation.

use core::{borrow::BorrowMut, fmt, str::FromStr};

use crate::{
    board::Role,
    game,
    position::{Check, Side},
    square::{File, Rank, Square},
};

use super::str_prelude::*;

#[derive(
    Clone, Copy, Debug, DeserializeFromStr, Eq, Ord, PartialEq, PartialOrd, SerializeDisplay,
)]
pub struct San {
    pub play: Move,
    pub check: Option<Check>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid SAN move")]
    Invalid,
    #[error("illegal SAN move")]
    Illegal,
    #[error("ambiguous SAN move")]
    Ambiguous,
}

#[derive(Debug, thiserror::Error)]
pub enum Apply {
    #[error(transparent)]
    San(#[from] Error),
    #[error(transparent)]
    Options(#[from] game::options::Error),
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

impl San {
    pub fn figurine(&self) -> String {
        let mut text = self.play.figurine();
        if let Some(check) = self.check {
            text.push_str(&check.to_string());
        }
        text
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

impl Move {
    fn figurine(&self) -> String {
        match *self {
            Move::Normal { role, file, rank, capture, to, promotion } => {
                let mut text = String::new();
                if role != Role::Pawn {
                    text.push(role.figurine());
                }
                if let Some(file) = file {
                    text.push_str(&file.to_string());
                }
                if let Some(rank) = rank {
                    text.push_str(&rank.to_string());
                }
                if capture {
                    text.push('x');
                }
                text.push_str(&to.to_string());
                if let Some(promotion) = promotion {
                    text.push('=');
                    text.push(promotion.figurine());
                }
                text
            }
            Move::Castle(Side::King) => "O-O".to_string(),
            Move::Castle(Side::Queen) => "O-O-O".to_string(),
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

impl crate::Move {
    pub fn san(self, short: game::Short, check: Option<Check>) -> San {
        let play = self;
        let play = if let Some(side) = play.castle_side() {
            Move::Castle(side)
        } else {
            let capture = play.capture.is_some() || play.is_en_passant();
            Move::Normal {
                role: play.role,
                file: if play.role == Role::Pawn && capture {
                    Some(play.from.file())
                } else {
                    short.file
                },
                rank: short.rank,
                capture,
                to: play.to,
                promotion: play.promotes(),
            }
        };

        San { play, check }
    }
}

impl game::MoveRef<'_> {
    pub fn san(&self) -> San {
        self.play().san(self.short(), self.position().check())
    }
}

impl game::MoveMut<'_> {
    pub fn san(&self) -> San {
        self.as_ref().san()
    }
}

impl<'g> game::PositionMut<'g> {
    pub fn push_san(&mut self, san: &San) -> Result<game::MoveMut<'_>, Apply> {
        let play = san.resolve(self.legal())?;
        Ok(self.push(play)?)
    }

    pub fn into_push_san(self, san: &San) -> Result<game::MoveMut<'g>, Apply> {
        let play = san.resolve(self.legal())?;
        Ok(self.into_push(play)?)
    }
}

impl<G: BorrowMut<crate::Game>> game::Cursor<G> {
    pub fn push_san(&mut self, san: &San) -> Result<game::MoveRef<'_>, Apply> {
        let play = san.resolve(self.position().legal())?;
        self.option_mut_or_push(play)?;
        Ok(self.play().expect("cursor advanced to move"))
    }

    pub fn append_sans(&mut self, sans: impl IntoIterator<Item = San>) -> Result<(), Apply> {
        for san in sans {
            self.push_san(&san)?;
        }
        Ok(())
    }
}

impl San {
    pub fn resolve(&self, legal: &[crate::Move]) -> Result<crate::Move, Error> {
        // We ignore incorrect check(mate) annotations.
        // Source disambiguation may be redundant, but must still match the move.
        let resolves_to_self = |play: &crate::Move| {
            let canonical = play.san(game::Short::new(legal, *play), None).play;
            match (self.play, canonical) {
                (Move::Castle(input), Move::Castle(actual)) => input == actual,
                (
                    Move::Normal { role, file, rank, capture, to, promotion },
                    Move::Normal {
                        role: actual_role,
                        capture: actual_capture,
                        to: actual_to,
                        promotion: actual_promotion,
                        ..
                    },
                ) => {
                    role == actual_role
                        && capture == actual_capture
                        && to == actual_to
                        && promotion == actual_promotion
                        && file.is_none_or(|file| file == play.from.file())
                        && rank.is_none_or(|rank| rank == play.from.rank())
                }
                _ => false,
            }
        };
        // legal crate moves that would match the input SAN move
        let mut matches = legal.iter().copied().filter(resolves_to_self);

        let Some(play) = matches.next() else {
            return Err(Error::Illegal);
        };
        if matches.next().is_some() {
            return Err(Error::Ambiguous);
        }
        Ok(play)
    }
}

impl FromStr for San {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut input = text.trim();
        let san = san(&mut input).map_err(|_| Error::Invalid)?;
        input.is_empty().then_some(san).ok_or(Error::Invalid)
    }
}

pub fn san(input: &mut Input<'_>) -> ModalResult<San> {
    let play = alt((castle, normal)).context(StrContext::Label("SAN move")).parse_next(input)?;
    let check = opt(check).context(StrContext::Label("SAN check")).parse_next(input)?;
    Ok(San { play, check })
}

fn castle(input: &mut Input<'_>) -> ModalResult<Move> {
    alt((queen_castle, king_castle)).context(StrContext::Label("SAN castle")).parse_next(input)
}

fn queen_castle(input: &mut Input<'_>) -> ModalResult<Move> {
    alt((
        "O-O-O".value(Move::Castle(Side::Queen)),
        "o-o-o".value(Move::Castle(Side::Queen)),
        "OOO".value(Move::Castle(Side::Queen)),
        "ooo".value(Move::Castle(Side::Queen)),
        "0-0-0".value(Move::Castle(Side::Queen)),
        "000".value(Move::Castle(Side::Queen)),
    ))
    .parse_next(input)
}

fn king_castle(input: &mut Input<'_>) -> ModalResult<Move> {
    alt((
        "O-O".value(Move::Castle(Side::King)),
        "o-o".value(Move::Castle(Side::King)),
        "OO".value(Move::Castle(Side::King)),
        "oo".value(Move::Castle(Side::King)),
        "0-0".value(Move::Castle(Side::King)),
        "00".value(Move::Castle(Side::King)),
    ))
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
        (None, None) => Err(backtrack()),
    }
}

fn square(input: &mut Input<'_>) -> ModalResult<Square> {
    (file, rank).map(|(file, rank)| Square::new(file, rank)).parse_next(input)
}

fn role(input: &mut Input<'_>) -> ModalResult<Role> {
    one_of(|c| "NBRQK".contains(c)).map(Role::panicky_from_char).parse_next(input)
}

fn promotion_role(input: &mut Input<'_>) -> ModalResult<Role> {
    one_of(|c| "NBRQnbrq".contains(c)).map(Role::panicky_from_char).parse_next(input)
}

fn promotion(input: &mut Input<'_>) -> ModalResult<Role> {
    opt('=').parse_next(input)?;
    promotion_role(input)
}

fn file(input: &mut Input<'_>) -> ModalResult<File> {
    one_of(|c| "abcdefgh".contains(c)).map(File::panicky_from_char).parse_next(input)
}

fn rank(input: &mut Input<'_>) -> ModalResult<Rank> {
    one_of(|c| "12345678".contains(c)).map(Rank::panicky_from_char).parse_next(input)
}

fn check(input: &mut Input<'_>) -> ModalResult<Check> {
    alt(('+'.value(Check::Check), '#'.value(Check::Checkmate))).parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        board::Role::*,
        square::{File::*, Square::*},
    };

    #[test]
    fn resolve_redundant_source_file() {
        let pgn = "1. d4 d5 2. Nc3 Nf6 3. Bf4 e6 4. e3 Bb4 5. Bd3 c5 6. dxc5 Nbd7 7. Nge2 Bxc5 *";
        assert!(crate::Game::from_pgn(pgn.parse().unwrap()).is_ok());
    }

    #[test]
    fn parse_pawn_play() {
        let parsed = san.parse("e4").unwrap();
        assert_eq!(
            parsed.play,
            Move::Normal {
                role: Pawn,
                file: None,
                rank: None,
                capture: false,
                to: E4,
                promotion: None,
            }
        );
    }

    #[test]
    fn parse_b_file_pawn_play() {
        let parsed = san.parse("b6").unwrap();
        assert_eq!(
            parsed.play,
            Move::Normal {
                role: Pawn,
                file: None,
                rank: None,
                capture: false,
                to: B6,
                promotion: None,
            }
        );
    }

    #[test]
    fn tolerate_whitespace() {
        let parsed = San::from_str("  O-O-O#  ").unwrap();
        assert_eq!(parsed.play, Move::Castle(Side::Queen));
        assert_eq!(parsed.check, Some(Check::Checkmate));
    }

    #[test]
    fn leave_padding_after_san() {
        assert!(san.parse("  O-O-O#  ").is_err());
    }

    #[test]
    fn parse_piece_play() {
        let parsed = san.parse("Nbd2+").unwrap();
        assert_eq!(
            parsed.play,
            Move::Normal {
                role: Knight,
                file: Some(B),
                rank: None,
                capture: false,
                to: D2,
                promotion: None,
            }
        );
        assert_eq!(parsed.check, Some(Check::Check));
    }

    #[test]
    fn parse_lowercase_user_input() {
        // Not doing this for now - accepting lowercase pieces
        // leads to ambiguity for e.g. `b3` which might be invalid `B3`.
        // assert_eq!(San::from_str("nf3").unwrap().to_string(), "Nf3");
        assert_eq!(San::from_str("exd8=q#").unwrap().to_string(), "exd8=Q#");
        assert_eq!(San::from_str("o-o").unwrap().to_string(), "O-O");
        assert_eq!(San::from_str("oo").unwrap().to_string(), "O-O");
        assert_eq!(San::from_str("ooo").unwrap().to_string(), "O-O-O");
        assert_eq!(San::from_str("00").unwrap().to_string(), "O-O");
        assert_eq!(San::from_str("000").unwrap().to_string(), "O-O-O");
    }

    #[test]
    fn parse_capture_promotion_and_castle() {
        let parsed = san.parse("exd8=Q#").unwrap();
        assert_eq!(
            parsed.play,
            Move::Normal {
                role: Pawn,
                file: Some(E),
                rank: None,
                capture: true,
                to: D8,
                promotion: Some(Queen),
            }
        );
        assert_eq!(parsed.check, Some(Check::Checkmate));

        let parsed = san.parse("O-O").unwrap();
        assert_eq!(parsed.play, Move::Castle(Side::King));

        let parsed = san.parse("0-0-0").unwrap();
        assert_eq!(parsed.play, Move::Castle(Side::Queen));
    }

    #[test]
    fn display_san() {
        assert_eq!(san.parse("Nbd2+").unwrap().to_string(), "Nbd2+");
        assert_eq!(san.parse("exd8=Q#").unwrap().to_string(), "exd8=Q#");
        assert_eq!(san.parse("O-O-O").unwrap().to_string(), "O-O-O");
        assert_eq!(san.parse("0-0").unwrap().to_string(), "O-O");
        assert_eq!(san.parse("exd1Q+").unwrap().to_string(), "exd1=Q+");
    }
}
