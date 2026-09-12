//! Chessbase header file format (CBH)

// https://talkchess.com/forum/viewtopic.php?topic_view=threads&p=287896
//
// cbh_codec.cpp, doOpen
//
// Header: 46 B
// Records: 46 B

use crate::formats::{Bits, ByteInput as Input, prelude::*};

use super::optional;

/// ChessBase header filename suffix.
pub const SUFFIX: &str = "cbh";

pub struct Index {
    pub header: Header,
    pub games: Vec<Game>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub software: Software,
    pub len: usize,
}

impl Header {
    pub const LEN: usize = 46;
}

// between 00 and 99
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Game {
    pub deleted: bool,
    pub offsets: Offsets,
    pub date: Date,
    pub outcome: Outcome,
    pub round: Option<u8>,
    pub subround: Option<u8>,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    pub medals: Medals,
    pub eco: Option<Eco>,
}

impl Game {
    pub const LEN: usize = 46;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Offsets {
    pub data: usize,       // -> cbg
    pub white: usize,      // -> cbp
    pub black: usize,      // -> cbp
    pub tournament: usize, // -> cbt
    pub source: usize,     // -> cbs

    pub annotation: Option<usize>, // -> cba
    pub annotator: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Date {
    pub year: Option<u16>,
    pub month: Option<u8>,
    pub day: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Eco {
    pub volume: Volume,
    pub number: Number,
    pub sub: Option<Number>,
}

// A - E
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Volume {
    A,
    B,
    C,
    D,
    E,
}

/// Between 00 and 99
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Number(pub u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Software {
    NineToEleven,
    Light,
    // corresponds to TWIC 1616
    Twic,
    Unknown([u8; 6]),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    // 0-1
    BlackWins,
    // 1/2-1/2
    Draw,
    // 1-0
    WhiteWins,
    // -:+
    WhiteBye,
    // =:=
    BothBye,
    // +:-
    BlackBye,
    // 0-0
    Lost,

    Unknown(u8),
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Medal {
    BestGame = 1 << 0,
    DecidedTournament = 1 << 1,
    ModelGame = 1 << 2,
    Novelty = 1 << 3,
    PawnStructure = 1 << 4,
    Strategy = 1 << 5,
    Tactics = 1 << 6,
    WithAttack = 1 << 7,
    Sacrifice = 1 << 8,
    Defense = 1 << 9,
    Material = 1 << 10,
    PiecePlay = 1 << 11,
    EndGame = 1 << 12,
    TacticalBlunder = 1 << 13,
    StrategicalBlunder = 1 << 14,
    User = 1 << 15,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Medals(pub u16);

impl Medals {
    #[inline]
    pub const fn contains(self, medal: Medal) -> bool {
        self.0 & medal as u16 != 0
    }
}

pub fn index(input: &mut Input<'_>) -> ModalResult<Index> {
    let header = header.parse_next(input)?;
    let games = repeat(header.len, game).parse_next(input)?;
    Ok(Index { header, games })
}

fn software(input: &mut Input<'_>) -> ModalResult<Software> {
    use Software::*;
    const CHESSBASE_9_10_11: &[u8] = &[0x00, 0x00, 0x2C, 0x00, 0x2E, 0x01];
    const CHESSBASE_LIGHT: &[u8] = &[0x00, 0x00, 0x24, 0x00, 0x2E, 0x01];
    // Content in TWIC 1616
    const CHESSBASE_X: &[u8] = &[0x00, 0x00, 0x2c, 0x00, 0x2E, 0x05];
    alt((
        CHESSBASE_9_10_11.value(NineToEleven),
        CHESSBASE_LIGHT.value(Light),
        CHESSBASE_X.value(Twic),
        take(6u8).try_map(|bytes: &[u8]| <[u8; 6]>::try_from(bytes)).map(Unknown),
    ))
    .parse_next(input)
}

pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    let software = software.parse_next(input)?;
    let len = be_u32.verify(|len| *len > 0).map(|len| len as usize - 1).parse_next(input)?;
    take(36u8).parse_next(input)?;

    Ok(Header { software, len })
}

// type Index = usize;

impl Date {
    pub fn pgn(self) -> String {
        let year = if let Some(year) = self.year { year.to_string() } else { "????".to_string() };
        let month =
            if let Some(month) = self.month { format!("{month:02}") } else { "??".to_string() };
        let day = if let Some(day) = self.day { format!("{day:02}") } else { "??".to_string() };
        format!("{year}.{month}.{day}")
    }
}

// fn be_u24(data: &[u8]) -> u32 {
//     let mut bytes = [0u8; 4];
//     bytes[1..].copy_from_slice(data);
//     u32::from_be_bytes(bytes)
// }

pub fn be_date(input: &mut Input<'_>) -> ModalResult<Date> {
    be_u24.map(date).parse_next(input)
}

pub fn le_date(input: &mut Input<'_>) -> ModalResult<Date> {
    le_u24.map(date).parse_next(input)
}

fn date(date: u32) -> Date {
    let year = (date >> 9) & ((1 << 12) - 1);
    let month = (date >> 5) & 0b1111;
    let day = date & 0b11111;
    Date { year: optional(year as u16), month: optional(month as u8), day: optional(day as u16) }
}

// #[repr(u8)]
// #[derive(Clone, Copy, Debug)]
// pub enum Wins {
//     Black,
//     White,
//     Draw,
// }

// impl Outcome {
//     pub fn wins(self) -> Wins {
//         BlackWins, WhiteBye => Black,
//         WhiteWins, BlackBye => White,
//         ...
//     }
// }

impl From<u8> for Outcome {
    fn from(outcome: u8) -> Self {
        use Outcome::*;
        match outcome {
            0 => BlackWins,
            1 => Draw,
            2 => WhiteWins,
            4 => WhiteBye,
            5 => BothBye,
            6 => BlackBye,
            7 => Lost,
            unknown => Unknown(unknown),
        }
    }
}

impl From<Outcome> for u8 {
    fn from(outcome: Outcome) -> Self {
        use Outcome::*;
        match outcome {
            BlackWins => 0,
            Draw => 1,
            WhiteWins => 2,
            WhiteBye => 4,
            BothBye => 5,
            BlackBye => 6,
            Lost => 7,
            Unknown(unknown) => unknown,
        }
    }
}

pub fn eco(input: &mut Input<'_>) -> ModalResult<Option<Eco>> {
    let mut bits = Bits::new(input);
    let code = bits.unsigned(9).ok_or_else(backtrack)?;
    if code > 500 {
        return Err(backtrack());
    };
    // Our Bits don't advance the underlying
    take(2u8).parse_next(input)?;

    // Convert
    Ok(if code == 0 {
        None
    } else {
        use Volume::*;
        let volume = match (code - 1) / 100 {
            0 => A,
            1 => B,
            2 => C,
            3 => D,
            4 => E,
            _ => unreachable!(),
        };
        let number = Number(((code - 1) % 100) as u8);
        // At least in TWIC 1616, this doesn't seem to be set
        let sub = bits.unsigned(7).ok_or_else(backtrack)?;
        if sub >= 100 {
            return Err(backtrack());
        }
        // assert!(sub == 0);
        // TODO: subcode in bits.unsigned(7) from 00 to 99
        Some(Eco { volume, number, sub: Some(Number(sub as u8)) })
    })
}

pub fn game(input: &mut Input<'_>) -> ModalResult<Game> {
    const GAME: u8 = 1 << 0;
    const GUIDING_TEXT: u8 = 1 << 1;
    const DELETED: u8 = 1 << 7;

    // Unhandled cases, cbh_codec.cpp Coddec::decodeIndex doesn't seem to handle these either
    let deleted = u8
        .verify(|meta| meta & GAME != 0 && meta & GUIDING_TEXT == 0)
        .map(|meta| meta & DELETED != 0)
        .parse_next(input)?;

    let data = be_u32.parse_next(input)? as usize;
    let annotation = be_u32.map(u32_as_usize).map(optional).parse_next(input)?;
    let white = be_u24.parse_next(input)? as usize;
    let black = be_u24.parse_next(input)? as usize;
    let tournament = be_u24.parse_next(input)? as usize;
    let annotator = be_u24.map(u32_as_usize).map(optional).parse_next(input)?;
    let source = be_u24.parse_next(input)? as usize;
    let offsets = Offsets { data, white, black, tournament, source, annotation, annotator };
    let date = be_date.parse_next(input)?;
    let outcome = u8.map(Outcome::from).parse_next(input)?;
    // skip line evaluation
    take(1u8).parse_next(input)?;

    let round = u8.map(optional).parse_next(input)?;
    let subround = u8.map(optional).parse_next(input)?;
    let white_elo = be_u16.map(optional).parse_next(input)?;
    let black_elo = be_u16.map(optional).parse_next(input)?;

    let eco = eco.parse_next(input)?;

    let medals = be_u16.map(Medals).parse_next(input)?;

    // skip some information
    // cf. https://talkchess.com/forum/viewtopic.php?topic_view=threads&p=287896
    // cf. cbh_codec.cpp
    take(7u8).parse_next(input)?;

    Ok(Game { deleted, offsets, date, outcome, round, subround, white_elo, black_elo, eco, medals })
}

fn u32_as_usize(x: u32) -> usize {
    x as usize
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::formats::twic;

    use super::*;

    const HEADER: [u8; Header::LEN] = hex!(
        "00002c002e050000186c000000000000
         00000000000000000000000000000000
         00000000000000000000186c0000"
    );
    const FIRST_GAME: [u8; Game::LEN] = hex!(
        "010000001a0000000000017f0002db00
         000e0000000000010fd355000003250a
         240ac7cd80000000000000000034"
    );

    #[test]
    fn header() {
        let mut input = HEADER.as_slice();
        let header = super::header.parse_next(&mut input).expect("can parse CBH header");

        assert_eq!(header.software, Software::Twic);
        assert_eq!(header.len, 6251);
        assert!(input.is_empty());
    }

    #[test]
    fn game() {
        let game = super::game.parse(FIRST_GAME.as_slice()).expect("can parse CBH game");

        assert_eq!(game.offsets.data, 26);
        assert_eq!(game.date, Date { year: Some(2025), month: Some(10), day: Some(21) });
        assert_eq!(game.outcome, Outcome::BlackWins);
        assert_eq!(game.round, Some(3));
        assert_eq!(game.subround, Some(37));
        assert_eq!(game.white_elo, Some(2596));
        assert_eq!(game.black_elo, Some(2759));
        assert_eq!(
            game.eco,
            Some(Eco { volume: Volume::E, number: Number(10), sub: Some(Number(0)) })
        );
    }

    #[test]
    #[ignore = "exhaustive comparison against the external TWIC fixture"]
    fn twic_index() {
        let twic = twic::data(1616, "cbh");

        let index = index.parse(twic.as_slice()).expect("can parse CBH index");

        assert_eq!(index.header.software, Software::Twic);
        assert_eq!(index.games.len(), index.header.len);
        assert_eq!(index.games.first().map(|game| game.offsets.data), Some(26));
        assert_eq!(index.games.first().map(|game| game.outcome), Some(Outcome::BlackWins));
        assert_eq!(index.games.last().map(|game| game.offsets.data), Some(633949));
    }

    #[test]
    fn unknown_outcome() {
        assert_eq!(Outcome::from(3), Outcome::Unknown(3));
        assert_eq!(Outcome::from(u8::MAX), Outcome::Unknown(u8::MAX));
        assert_eq!(u8::from(Outcome::Unknown(3)), 3);
        assert_eq!(u8::from(Outcome::Unknown(u8::MAX)), u8::MAX);
    }
}
