//! Chessbase header file format (CBH)

// https://talkchess.com/forum/viewtopic.php?topic_view=threads&p=287896
//
// cbh_codec.cpp, doOpen
//
// Header: 46 B
// Records: 46 B

use enumflags2::{BitFlags, bitflags};

use super::{Bits, ByteInput as Input, prelude::*};

// numbers where 0 mean missing or unknown
pub(crate) fn optional<T: Default + PartialEq>(x: T) -> Option<T> {
    (x != T::default()).then_some(x)
}

fn u32_as_usize(x: u32) -> usize {
    x as usize
}

#[derive(Clone, Copy, Debug)]
pub enum Creator {
    Chessbase9_10_11,
    ChessbaseLight,
    // corresponds to TWIC 1616
    ChessbaseX,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub creator: Creator,
    pub len: usize,
}

pub struct Headers {
    pub header: Header,
    pub records: Vec<Record>,
}

pub fn headers(input: &mut Input<'_>) -> ModalResult<Headers> {
    let header = header.parse_next(input)?;
    let records: Vec<Record> = repeat(..=header.len, record).parse_next(input)?;
    assert_eq!(records.len(), header.len);
    Ok(Headers { header, records })
}

#[test]
fn example() {
    let input = include_bytes!("../../examples/twic1616.cbh");
    header.parse_next(&mut input.as_slice()).expect("can pass CBH header");

    let cbh = headers.parse(input).expect("can parse CBH file");
    println!("{:?}", cbh.header);
    for game in cbh.records.iter().take(3) {
        println!("{game:?}");
        println!("{}", game.date.pgn());
    }
    for game in cbh.records.iter().rev().take(3) {
        println!("{game:?}");
        println!("{}", game.date.pgn());
    }
}

fn creator(input: &mut Input<'_>) -> ModalResult<Creator> {
    use Creator::*;
    const CHESSBASE_9_10_11: &[u8] = &[0x00, 0x00, 0x2C, 0x00, 0x2E, 0x01];
    const CHESSBASE_LIGHT: &[u8] = &[0x00, 0x00, 0x24, 0x00, 0x2E, 0x01];
    // Content in TWIC 1616
    const CHESSBASE_X: &[u8] = &[0x00, 0x00, 0x2c, 0x00, 0x2E, 0x05];
    alt((
        CHESSBASE_9_10_11.value(Chessbase9_10_11),
        CHESSBASE_LIGHT.value(ChessbaseLight),
        CHESSBASE_X.value(ChessbaseX),
        take(6u8)
            .map(|bytes| {
                /*println!("{}", hex::encode(bytes));*/
                bytes
            })
            .value(Unknown),
    ))
    .parse_next(input)
}

pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    assert!(input.len() >= 46);
    let creator = creator.parse_next(input).unwrap();
    let len = be_u32.map(|len_plus_one: u32| len_plus_one as usize - 1).parse_next(input)?; //.unwrap();
    take(36u8).parse_next(input)?;
    // let (creator, games) = terminated((creator, dec_uint.map(|games: u32| games as usize - 1)), take(36u8))
    //     .parse_next(input).unwrap();//?;

    Ok(Header { creator, len })
}

// type Index = usize;

#[derive(Clone, Copy, Debug)]
pub struct Meta {
    pub game: bool,
    pub guiding_text: bool,
    pub deleted: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Date {
    pub year: Option<u16>,
    pub month: Option<u8>,
    pub day: Option<u16>,
}

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
    let date = be_u24.parse_next(input)?;
    let year = (date >> 9) & ((1 << 12) - 1);
    let month = (date >> 5) & 0b1111;
    let day = date & 0b11111;
    Ok(Date {
        year: optional(year as u16),
        month: optional(month as u8),
        day: optional(day as u16),
    })
}

pub fn le_date(input: &mut Input<'_>) -> ModalResult<Date> {
    let date = le_u24.parse_next(input)?;
    let year = (date >> 9) & ((1 << 12) - 1);
    let month = (date >> 5) & 0b1111;
    let day = date & 0b11111;
    Ok(Date {
        year: optional(year as u16),
        month: optional(month as u8),
        day: optional(day as u16),
    })
}

// #[repr(u8)]
// #[derive(Clone, Copy, Debug)]
// pub enum Wins {
//     Black,
//     White,
//     Draw,
// }

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Outcome {
    // 1-0
    BlackWins = 0,
    // 1/2-1/2
    Draw = 1,
    // 1-1
    WhiteWins = 2,
    // -:+
    WhiteBye = 4,
    // =:=
    BothBye = 5,
    // +:-
    BlackBye = 6,
    // 0-0
    Lost = 7,

    Unknown,
}

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
            _ => Unknown,
        }
    }
}

#[bitflags]
#[repr(u16)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Flag {
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

pub type Flags = BitFlags<Flag>;

// impl Flag {
//     pub fn parse(flags: u16) -> Set<Flag> {
//         use Flag::*;

//         let mut set = Set::new();
//         if flags & (1 <<  0) != 0 { set.insert(BestGame); };
//         if flags & (1 <<  1) != 0 { set.insert(DecidedTournament); };
//         if flags & (1 <<  2) != 0 { set.insert(ModelGame); };
//         if flags & (1 <<  3) != 0 { set.insert(Novelty); };
//         if flags & (1 <<  4) != 0 { set.insert(PawnStructure); };
//         if flags & (1 <<  5) != 0 { set.insert(Strategy); };
//         if flags & (1 <<  6) != 0 { set.insert(Tactics); };
//         if flags & (1 <<  7) != 0 { set.insert(WithAttack); };
//         if flags & (1 <<  8) != 0 { set.insert(Sacrifice); };
//         if flags & (1 <<  9) != 0 { set.insert(Defense); };
//         if flags & (1 << 10) != 0 { set.insert(Material); };
//         if flags & (1 << 11) != 0 { set.insert(PiecePlay); };
//         if flags & (1 << 12) != 0 { set.insert(EndGame); };
//         if flags & (1 << 13) != 0 { set.insert(TacticalBlunder); };
//         if flags & (1 << 14) != 0 { set.insert(StrategicalBlunder); };
//         if flags & (1 << 15) != 0 { set.insert(User); };
//         set
//     }
// }

pub mod eco {
    use super::*;

    // A - E
    #[derive(Clone, Copy, Debug)]
    pub enum Volume {
        A,
        B,
        C,
        D,
        E,
    }

    /// Between 00 and 99
    #[derive(Clone, Copy, Debug)]
    pub struct Number(pub u8);

    #[derive(Clone, Copy, Debug)]
    pub struct Eco {
        pub volume: Volume,
        pub number: Number,
        pub sub: Option<Number>,
    }

    pub fn eco(input: &mut Input<'_>) -> ModalResult<Option<Eco>> {
        use winnow::error::{ContextError, ErrMode};

        let err = || ErrMode::Backtrack(ContextError::new());

        let mut bits = Bits::new(input);
        let code = bits.unsigned(9).ok_or_else(err)?;
        if code > 500 {
            return Err(err());
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
            let sub = bits.unsigned(7).ok_or_else(err)?;
            assert!(sub < 100);
            // assert!(sub == 0);
            // TODO: subcode in bits.unsigned(7) from 00 to 99
            Some(Eco { volume, number, sub: Some(Number(sub as u8)) })
        })
    }
}

pub use eco::{Eco, eco};

// between 00 and 99
#[derive(Clone, Copy, Debug)]
pub struct Record {
    pub meta: Meta,
    pub game_offset: usize,               // -> cbg
    pub annotation_offset: Option<usize>, // -> cba
    pub white: usize,                     // -> cbp
    pub black: usize,                     // -> cbp
    pub tournament: usize,                // -> cbt
    pub annotator: Option<usize>,
    pub source: usize,
    pub date: Date,
    pub outcome: Outcome,
    pub round: Option<u8>,
    pub subround: Option<u8>,
    pub white_elo: Option<u16>,
    pub black_elo: Option<u16>,
    pub medals: Flags,
    pub eco: Option<Eco>,
}

pub fn record(input: &mut Input<'_>) -> ModalResult<Record> {
    let meta = u8.parse_next(input)?;
    let meta = Meta {
        game: meta & (1 << 0) != 0,
        guiding_text: meta & (1 << 1) != 0,
        deleted: meta & (1 << 7) != 0,
    };
    // Unhandled cases, cbh_codec.cpp Coddec::decodeIndex doesn't seem to handle these either
    assert!(meta.game);
    assert!(!meta.guiding_text);

    let game_offset = be_u32.parse_next(input)? as usize;
    let annotation_offset = be_u32.map(u32_as_usize).map(optional).parse_next(input)?;
    let white = be_u24.parse_next(input)? as usize;
    let black = be_u24.parse_next(input)? as usize;
    let tournament = be_u24.parse_next(input)? as usize;
    let annotator = be_u24.map(u32_as_usize).map(optional).parse_next(input)?;
    let source = be_u24.parse_next(input)? as usize;
    let date = be_date.parse_next(input)?;
    let outcome = u8.map(Outcome::from).parse_next(input)?;
    // skip line evaluation
    take(1u8).parse_next(input)?;

    let round = u8.map(optional).parse_next(input)?;
    let subround = u8.map(optional).parse_next(input)?;
    let white_elo = be_u16.map(optional).parse_next(input)?;
    let black_elo = be_u16.map(optional).parse_next(input)?;

    let eco = eco.parse_next(input)?;

    let medals = be_u16
        .map(|flags| Flags::from_bits(flags).expect("all bits are legal"))
        .parse_next(input)?;

    // skip some information
    // cf. https://talkchess.com/forum/viewtopic.php?topic_view=threads&p=287896
    // cf. cbh_codec.cpp
    take(7u8).parse_next(input)?;

    Ok(Record {
        meta,
        game_offset,
        annotation_offset,
        white,
        black,
        tournament,
        annotator,
        source,
        date,
        outcome,
        round,
        subround,
        white_elo,
        black_elo,
        eco,
        medals,
    })
}

#[test]
fn cbh() {
    let example = include_bytes!("../../examples/twic1616.cbh");
    let input = &mut example.as_slice();
    let header = header.parse_next(input).unwrap();
    println!("{header:?}");
    for _ in 0..3 {
        let record = record.parse_next(input).unwrap();
        println!("{record:?}");
    }
    // panic!("wtf");
}
