//! Chessbase player file format

use crate::formats::bytes_prelude::*;

use super::{
    common::{Header, Node, header, node},
    text,
};

/// ChessBase player filename suffix.
pub const SUFFIX: &str = "cbp";

#[derive(Clone, Debug)]
pub struct Player {
    pub deleted: bool,
    pub first_name: Option<Text>,
    pub last_name: Option<Text>,
    pub games: usize,
    pub first_game: usize,
}

pub fn player(input: &mut Input<'_>) -> ModalResult<Player> {
    let Node { deleted } = node.parse_next(input)?;
    let last_name = take(30u8).map(text).parse_next(input)?;
    let first_name = take(20u8).map(text).parse_next(input)?;
    let games = le_u32.parse_next(input)? as usize;
    let first_game = le_u32.parse_next(input)? as usize;

    Ok(Player { deleted, first_name, last_name, games, first_game })
}

#[derive(Clone, Debug)]
pub struct Players {
    pub header: Header,
    pub players: Vec<Player>,
}

pub fn players(input: &mut Input<'_>) -> ModalResult<Players> {
    let header = header.parse_next(input)?;
    let players: Vec<Player> = repeat(header.len, player).parse_next(input)?;
    Ok(Players { header, players })
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::formats::twic;

    use super::*;

    #[test]
    #[ignore = "exploratory output from the external TWIC fixture"]
    fn twic_players() {
        let input = twic::data(1616, "cbp");
        let header = header.parse_next(&mut input.as_slice()).expect("can parse CBP header");
        println!("{header:?}");
        let players = players.parse(input.as_slice()).map_err(drop).expect("can parse CBP file");
        println!("{:?}", players.header);
        for player in players.players.iter().take(3) {
            println!("{player:?}");
        }
        for player in players.players.iter().rev().take(3) {
            println!("{player:?}");
        }
    }

    #[test]
    fn player() {
        const INPUT: [u8; 67] = hex!(
            "ffffffffffffffff0041616761617264
             00000000360038002500000000000080
             000000009e86004a00004d003a007000
             6d0000000000000000000005000000f0
             0b0000"
        );

        let player = super::player.parse(INPUT.as_slice()).expect("can parse CBP player");
        assert!(!player.deleted);
        assert_eq!(player.first_name.as_deref(), Some("J"));
        assert_eq!(player.last_name.as_deref(), Some("Aagaard"));
        assert_eq!(player.games, 5);
        assert_eq!(player.first_game, 3056);
    }
}
