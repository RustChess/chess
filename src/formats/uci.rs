//! Universal Chess Interface notation.

use core::{fmt, str::FromStr};

use crate::{
    Role,
    position::{File, Rank, Square},
};

use super::{StrInput as Input, prelude::*};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(SerializeDisplay, DeserializeFromStr))]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub promotion: Option<Role>,
}

impl From<crate::Move> for Move {
    // TODO: This is only valid for standard chess.
    // In Freestyle, castling is a move to the rook.
    fn from(play: crate::Move) -> Self {
        Self { from: play.from, to: play.to, promotion: play.promotes() }
    }
}

impl crate::Move {
    pub fn uci(self) -> Move {
        self.into()
    }
}

impl Move {
    pub fn resolve(self, legal: &[crate::Move]) -> Option<crate::Move> {
        legal.iter().copied().find(|play| {
            play.from == self.from && play.to == self.to && play.promotes() == self.promotion
        })
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.from.fmt(f)?;
        self.to.fmt(f)?;
        if let Some(promotion) = self.promotion {
            promotion.lower().fmt(f)?;
        }
        Ok(())
    }
}

impl FromStr for Move {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut input = s.trim();
        let play = uci_move(&mut input).map_err(|_| Error::InvalidMove)?;
        input.is_empty().then_some(play).ok_or(Error::InvalidMove)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid UCI move")]
    InvalidMove,
    #[error("invalid UCI info")]
    InvalidInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pv {
    pub moves: Vec<Move>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Info {
    pub depth: Option<u32>,
    pub seldepth: Option<u32>,
    pub multipv: Option<usize>,
    pub nodes: Option<u32>,
    pub score: Option<Score>,
    pub bound: Option<Bound>,
    pub pv: Option<Pv>,
}

impl FromStr for Info {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut input = s.trim();
        let info = info(&mut input).map_err(|_| Error::InvalidInfo)?;
        input.is_empty().then_some(info).ok_or(Error::InvalidInfo)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Score {
    Centipawns(i32),
    Mate(i32),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bound {
    Lower,
    Upper,
}

/// Parse a UCI move and resolve it against legal moves.
pub fn parse_move(text: &str, legal: &[crate::Move]) -> Option<crate::Move> {
    let mut input = text.trim();
    let play = uci_move(&mut input).ok()?;
    input.is_empty().then_some(())?;

    play.resolve(legal)
}

pub fn uci_move(input: &mut Input<'_>) -> ModalResult<Move> {
    (square, square, opt(promotion))
        .map(|(from, to, promotion)| Move { from, to, promotion })
        .parse_next(input)
}

pub fn info(input: &mut Input<'_>) -> ModalResult<Info> {
    "info".parse_next(input)?;

    let mut info = Info {
        depth: None,
        seldepth: None,
        multipv: None,
        nodes: None,
        score: None,
        bound: None,
        pv: None,
    };

    while preceded(space1, |input: &mut Input<'_>| info_field(input, &mut info))
        .parse_next(input)
        .is_ok()
    {}

    Ok(info)
}

fn info_field(input: &mut Input<'_>, info: &mut Info) -> ModalResult<()> {
    alt((
        "depth".value(Field::Depth),
        "seldepth".value(Field::Seldepth),
        "multipv".value(Field::Multipv),
        "nodes".value(Field::Nodes),
        "score".value(Field::Score),
        "lowerbound".value(Field::Lowerbound),
        "upperbound".value(Field::Upperbound),
        "pv".value(Field::Pv),
        token.value(Field::Skip),
    ))
    .parse_next(input)?
    .parse(info, input)
}

#[derive(Clone, Copy)]
enum Field {
    Depth,
    Seldepth,
    Multipv,
    Nodes,
    Score,
    Lowerbound,
    Upperbound,
    Pv,
    Skip,
}

impl Field {
    fn parse(self, info: &mut Info, input: &mut Input<'_>) -> ModalResult<()> {
        match self {
            Field::Depth => info.depth = Some(preceded(space1, dec_uint).parse_next(input)?),
            Field::Seldepth => info.seldepth = Some(preceded(space1, dec_uint).parse_next(input)?),
            Field::Multipv => info.multipv = Some(preceded(space1, dec_uint).parse_next(input)?),
            Field::Nodes => info.nodes = Some(preceded(space1, dec_uint).parse_next(input)?),
            Field::Score => info.score = Some(preceded(space1, score).parse_next(input)?),
            Field::Lowerbound => info.bound = Some(Bound::Lower),
            Field::Upperbound => info.bound = Some(Bound::Upper),
            Field::Pv => info.pv = Some(preceded(space1, pv).parse_next(input)?),
            Field::Skip => {}
        }

        Ok(())
    }
}

fn token(input: &mut Input<'_>) -> ModalResult<()> {
    take_till(1.., char::is_whitespace).void().parse_next(input)
}

fn score(input: &mut Input<'_>) -> ModalResult<Score> {
    alt((
        preceded(("cp", space1), signed).map(Score::Centipawns),
        preceded(("mate", space1), signed).map(Score::Mate),
    ))
    .parse_next(input)
}

fn signed(input: &mut Input<'_>) -> ModalResult<i32> {
    (opt(alt(('+'.value(1), '-'.value(-1)))), dec_uint)
        .verify_map(|(sign, value): (Option<i32>, u32)| {
            i32::try_from(value).ok().map(|value| sign.unwrap_or(1) * value)
        })
        .parse_next(input)
}

fn pv(input: &mut Input<'_>) -> ModalResult<Pv> {
    separated(1.., uci_move, space1).map(|moves| Pv { moves }).parse_next(input)
}

fn square(input: &mut Input<'_>) -> ModalResult<Square> {
    (file, rank).map(|(file, rank)| Square::new(file, rank)).parse_next(input)
}

fn file(input: &mut Input<'_>) -> ModalResult<File> {
    one_of(|c| "abcdefgh".contains(c)).map(File::panicky_from_char).parse_next(input)
}

fn rank(input: &mut Input<'_>) -> ModalResult<Rank> {
    one_of(|c| "12345678".contains(c)).map(Rank::panicky_from_char).parse_next(input)
}

fn promotion(input: &mut Input<'_>) -> ModalResult<Role> {
    one_of(|c| "nbrqNBRQ".contains(c)).map(Role::panicky_from_char).parse_next(input)
}

#[cfg(test)]
mod tests {
    use crate::{
        Position,
        position::{Role::*, Square::*},
        variant::Chess,
    };

    use super::*;

    #[test]
    fn parses_normal_move() {
        let legal = Position::start().legal_moves();

        assert_eq!(parse_move("e2e4", &legal).unwrap().to, E4);
        assert!(parse_move("e2e5", &legal).is_none());
    }

    #[test]
    fn parses_promotion_move() {
        let position: Position<Chess> = Position::from_fen("8/P7/8/8/8/8/8/k6K w - - 0 1").unwrap();
        let legal = position.legal_moves();

        assert_eq!(parse_move("a7a8q", &legal).unwrap().promotes(), Some(Queen));
        assert_eq!(parse_move("a7a8Q", &legal).unwrap().promotes(), Some(Queen));
        assert!(parse_move("a7a8", &legal).is_none());
    }

    #[test]
    fn resolves_special_moves_from_legal_moves() {
        let castle: Position<Chess> =
            Position::from_fen("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1").unwrap();
        assert!(parse_move("e1g1", &castle.legal_moves()).is_some_and(crate::Move::is_castle));

        let en_passant: Position<Chess> =
            Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
        assert!(
            parse_move("e5d6", &en_passant.legal_moves()).is_some_and(crate::Move::is_en_passant)
        );
    }

    #[test]
    fn parses_info() {
        let parsed = info
            .parse(
                "info depth 24 seldepth 47 multipv 1 score cp 30 nodes 46777504 nps 15592501 hashfull 1000 tbhits 0 time 3000 pv d2d4 g8f6",
            )
            .unwrap();

        assert_eq!(parsed.depth, Some(24));
        assert_eq!(parsed.seldepth, Some(47));
        assert_eq!(parsed.multipv, Some(1));
        assert_eq!(parsed.nodes, Some(46777504));
        assert_eq!(parsed.score, Some(Score::Centipawns(30)));
        assert_eq!(parsed.bound, None);
        assert_eq!(
            parsed.pv,
            Some(Pv {
                moves: vec![
                    Move { from: D2, to: D4, promotion: None },
                    Move { from: G8, to: F6, promotion: None },
                ],
            })
        );
    }

    #[test]
    fn parses_info_mate_and_bound() {
        let parsed = info
            .parse("info depth 6 score mate -3 upperbound nodes 974 pv c8c1 d2f1 c1f1")
            .unwrap();

        assert_eq!(parsed.depth, Some(6));
        assert_eq!(parsed.score, Some(Score::Mate(-3)));
        assert_eq!(parsed.bound, Some(Bound::Upper));
        assert_eq!(parsed.nodes, Some(974));
        assert_eq!(parsed.pv.unwrap().moves.len(), 3);
    }
}
