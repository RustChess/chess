//! Chessbase tournament file format (CBT)

use crate::formats::{ByteInput as Input, prelude::*};

use super::{
    cstr,
    index::{Date, le_date},
    optional,
    table::{Header, header},
};

/// ChessBase tournament filename suffix.
pub const SUFFIX: &str = "cbt";

#[derive(Clone, Debug)]
pub struct Tournament {
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

pub fn tournament(input: &mut Input<'_>) -> ModalResult<Tournament> {
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
    let (mode, speed) = u8.verify_map(byte83).parse_next(input)?;

    let _byte84 = u8.parse_next(input)?;
    let nation = u8.map(optional).parse_next(input)?;

    let _byte86 = u8.parse_next(input)?;
    let category = u8.verify(|x| *x < 100).map(optional).parse_next(input)?;

    let _byte88 = u8.parse_next(input)?;
    let rounds = u8.map(optional).parse_next(input)?;
    let _byte90 = u8.parse_next(input)?;
    let games = le_u32.parse_next(input)? as usize;
    let first_game = le_u32.parse_next(input)? as usize;

    Ok(Tournament {
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

fn byte83(byte: u8) -> Option<(Option<Mode>, Speed)> {
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
            _ => return None,
        })
    };
    let blitz = (byte >> 5) & 1 != 0;
    let rapid = (byte >> 6) & 1 != 0;
    let correspondence = (byte >> 7) & 1 != 0;
    if blitz as u8 + rapid as u8 + correspondence as u8 > 1 {
        return None;
    }
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

    Some((mode, speed))
}

#[derive(Clone, Debug)]
pub struct Tournaments {
    pub header: Header,
    pub tournaments: Vec<Tournament>,
}

pub fn tournaments(input: &mut Input<'_>) -> ModalResult<Tournaments> {
    let header = header.verify(|header| header.record_len == 90).parse_next(input)?;
    let tournaments: Vec<Tournament> = repeat(..=header.len, tournament).parse_next(input)?;
    Ok(Tournaments { header, tournaments })
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::formats::twic;

    use super::*;

    #[test]
    #[ignore = "exploratory output from the external TWIC fixture"]
    fn twic_tournaments() {
        let input = twic::data(1616, "cbt");
        let header = header.parse_next(&mut input.as_slice()).expect("can parse CBT header");
        println!("{header:?}");
        let tournaments =
            tournaments.parse(input.as_slice()).map_err(drop).expect("can parse CBT file");
        println!("{:?}", tournaments.header);
        for tournament in tournaments.tournaments.iter().take(5) {
            println!("{tournament:?}");
        }
        for tournament in tournaments.tournaments.iter().rev().take(5) {
            println!("{tournament:?}");
        }
    }

    #[test]
    fn tournament() {
        const INPUT: [u8; 99] = hex!(
            "ffffffffffffffff0031327468204265
             69727574204f70656e20323032350068
             0000009f0dde010000e8f9e176de0100
             00426569727574204c424e0053500072
             74204155540001000080d8c0b9f87f52
             d30f000400520000000900f8000000d6
             070000"
        );

        let tournament =
            super::tournament.parse(INPUT.as_slice()).expect("can parse CBT tournament");
        assert_eq!(tournament.title, "12th Beirut Open 2025");
        assert_eq!(tournament.place, "Beirut LBN");
        assert_eq!(tournament.date, Date { year: Some(2025), month: Some(10), day: Some(18) });
        assert!(matches!(tournament.mode, Some(Mode::Swiss)));
        assert!(matches!(tournament.speed, Some(Speed::Normal)));
        assert_eq!(tournament.nation, Some(82));
        assert_eq!(tournament.category, None);
        assert_eq!(tournament.rounds, Some(9));
        assert_eq!(tournament.games, 248);
        assert_eq!(tournament.first_game, 2006);
    }
}
