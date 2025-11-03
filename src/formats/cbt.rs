//! Chessbase tournament file format (CBT)

use super::{cbv::cstr, prelude::*};

use super::cbh::optional;
pub use super::{
    cbh::{Date, le_date},
    cbp::{Header, header},
};

#[derive(Clone, Debug)]
pub struct Record {
    pub title: String,
    pub place: String,
    pub date: Date,
    pub mode: Option<Mode>,
    pub speed: Option<Speed>,
    pub nation: Option<u8>,
    pub category: Option<u8>,
    pub rounds: Option<u8>,
    pub games: usize,
    pub first_game: usize,
}

pub fn record(input: &mut Input<'_>) -> ModalResult<Record> {
    let _what = be_u16.parse_next(input)?;
    // 0x19FC = deleted? 0xFFFF = exist?
    // In the TWIC file, we see all these values:
    // 0, 100, ffff, 200, 300, d00, ...
    // println!("{_what:x}");
    // assert!(what == 0x19FC || what == 0xFFFF);
    // assert!(what == 0x19FC || what == 0xFFFF);
    let _ = take(7u8).parse_next(input)?;

    let title = take(40u8).map(cstr).parse_next(input)?;
    let place = take(30u8).map(cstr).parse_next(input)?;

    let date = le_date.parse_next(input)?;

    let _byte82 = u8.parse_next(input)?;
    let (mode, speed) = u8.map(byte83).parse_next(input)?;

    let _byte84 = u8.parse_next(input)?;
    let nation = u8.map(optional).parse_next(input)?;

    let _byte86 = u8.parse_next(input)?;
    let category = u8.verify(|x| *x < 100).map(optional).parse_next(input)?;

    let _byte88 = u8.parse_next(input)?;
    let rounds = u8.map(optional).parse_next(input)?;
    let _byte90 = u8.parse_next(input)?;
    let games = le_u32.parse_next(input)? as usize;
    let first_game = le_u32.parse_next(input)? as usize;

    Ok(Record {
        title,
        place,
        date,
        mode,
        speed: Some(speed),
        nation,
        category,
        rounds,
        games,
        first_game,
    })
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Mode {
    Game = 1,
    Match = 2,
    Tournament = 3,
    Swiss = 4,
    Team = 5,
    Knockout = 6,
    Simultaneous = 7,
    Scheveningen = 8,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Speed {
    Normal,
    Blitz,
    Rapid,
    Correspondence,
}

fn byte83(byte: u8) -> (Option<Mode>, Speed) {
    let mode = byte & 0b11111;
    let mode = if mode == 0 {
        None
    } else {
        use Mode::*;
        Some(match mode {
            1 => Game,
            2 => Match,
            3 => Tournament,
            4 => Swiss,
            5 => Team,
            6 => Knockout,
            7 => Simultaneous,
            8 => Scheveningen,
            _ => unreachable!(),
        })
    };
    let blitz = (byte >> 5) & 1 != 0;
    let rapid = (byte >> 6) & 1 != 0;
    let correspondence = (byte >> 7) & 1 != 0;
    assert!(blitz as u8 + rapid as u8 + correspondence as u8 <= 1);
    let mut speed = Speed::Normal;
    if blitz {
        speed = Speed::Blitz
    };
    if rapid {
        speed = Speed::Rapid
    };
    if correspondence {
        speed = Speed::Correspondence
    };

    (mode, speed)
}

#[derive(Clone, Debug)]
pub struct Tournaments {
    pub header: Header,
    pub records: Vec<Record>,
}

pub fn tournaments(input: &mut Input<'_>) -> ModalResult<Tournaments> {
    let header = header.parse_next(input)?;
    assert_eq!(header.record_len + 9, 99);
    let records: Vec<Record> = repeat(..=header.len, record).parse_next(input)?;
    Ok(Tournaments { header, records })
}

#[test]
fn example() {
    let input = include_bytes!("../../examples/twic1616.cbt");
    let header = header.parse_next(&mut input.as_slice()).expect("can parse CBT header");
    println!("{header:?}");
    let tournaments =
        tournaments.parse(input.as_slice()).map_err(drop).expect("can parse CBT file");
    println!("{:?}", tournaments.header);
    for record in tournaments.records.iter().take(5) {
        println!("{record:?}");
    }
    for record in tournaments.records.iter().rev().take(5) {
        println!("{record:?}");
    }
}
