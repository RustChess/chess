//! ChessBase team file format.

use crate::formats::bytes_prelude::*;

use super::{
    common::{Header, Node, header, node},
    optional, text,
};

/// ChessBase team filename suffix.
pub const SUFFIX: &str = "cbe";

#[derive(Clone, Debug)]
pub struct Team {
    pub deleted: bool,
    pub title: Option<Text>,
    pub number: Option<u32>,
    pub year: Option<u16>,
    pub season: bool,
    pub nation: Option<u8>,
    pub games: usize,
    pub first_game: usize,
}

pub fn team(input: &mut Input<'_>) -> ModalResult<Team> {
    let Node { deleted } = node.parse_next(input)?;
    let title = take(45u8).map(text).parse_next(input)?;
    let number = le_u32.map(optional).parse_next(input)?;
    let season = u8.map(|year2| year2 != 0).parse_next(input)?;
    let year = le_u16.map(optional).parse_next(input)?;
    take(2u8).parse_next(input)?;
    let nation = u8.map(optional).parse_next(input)?;
    let games = le_u32.parse_next(input)? as usize;
    let first_game = le_u32.parse_next(input)? as usize;

    Ok(Team { deleted, title, number, year, season, nation, games, first_game })
}

#[derive(Clone, Debug)]
pub struct Teams {
    pub header: Header,
    pub teams: Vec<Team>,
}

pub fn teams(input: &mut Input<'_>) -> ModalResult<Teams> {
    let header = header.verify(|header| header.record_len == 63).parse_next(input)?;
    let teams = repeat(header.len, team).parse_next(input)?;
    Ok(Teams { header, teams })
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::formats::twic;

    use super::*;

    #[test]
    fn team() {
        const INPUT: [u8; 72] = hex!(
            "ffffffffffffffff004161746f730061
             6863652053617472616e632053706f72
             204b756c7562750053616b6b204b6c75
             62000073005c0000000000e907000000
             0600000051010000"
        );

        let team = super::team.parse(INPUT.as_slice()).expect("can parse CBE team");
        assert!(!team.deleted);
        assert_eq!(team.title.as_deref(), Some("Aatos"));
        assert_eq!(team.number, None);
        assert_eq!(team.year, Some(2025));
        assert!(!team.season);
        assert_eq!(team.nation, None);
        assert_eq!(team.games, 6);
        assert_eq!(team.first_game, 337);
    }

    #[test]
    #[ignore = "exploratory output from the external TWIC fixture"]
    fn twic_teams() {
        let input = twic::data(1616, "cbe");
        let teams = teams.parse(input.as_slice()).map_err(drop).expect("can parse CBE file");
        println!("{:?}", teams.header);
        for team in teams.teams.iter().take(5) {
            println!("{team:?}");
        }
    }
}
