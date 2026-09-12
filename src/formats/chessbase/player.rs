//! Chessbase player file format

use crate::formats::{ByteInput as Input, prelude::*};

use super::{
    cstr,
    table::{Header, header},
};

/// ChessBase player filename suffix.
pub const SUFFIX: &str = "cbp";

// numbers where -1 mean missing or unknown
// fn optional1<T: Default + PartialEq>(x: T) -> Option<T> {
//     (x != T::default()).then_some(x)
// }
fn optional1(x: i32) -> Option<Option<usize>> {
    match x {
        -1 => Some(None),
        0.. => Some(Some(x as usize)),
        _ => None,
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub deleted: bool,
    pub first_name: String,
    pub last_name: String,
    pub games: usize,
    pub first_game: usize,
}

pub fn player(input: &mut Input<'_>) -> ModalResult<Player> {
    let (left_child, deleted) =
        le_i32
            .verify_map(|x| {
                if x == -999 { Some((None, true)) } else { optional1(x).map(|x| (x, false)) }
            })
            .parse_next(input)?;
    let (right_child, next_deleted) = le_i32
        .verify_map(|x| {
            if deleted { optional1(x).map(|x| (None, x)) } else { optional1(x).map(|x| (x, None)) }
        })
        .parse_next(input)?;
    let _right_height_minus_left_height =
        i8.verify(|height| !deleted || *height == 0).parse_next(input)?;
    let last_name = take(30u8).map(cstr).parse_next(input)?;
    let first_name = take(20u8).map(cstr).parse_next(input)?;
    let games = le_u32.parse_next(input)? as usize;
    let first_game = le_u32.parse_next(input)? as usize;

    let _ = (left_child, right_child, next_deleted);

    Ok(Player { deleted, first_name, last_name, games, first_game })
}

#[derive(Clone, Debug)]
pub struct Players {
    pub header: Header,
    pub players: Vec<Player>,
}

pub fn players(input: &mut Input<'_>) -> ModalResult<Players> {
    let header = header.parse_next(input)?;
    let players: Vec<Player> = repeat(..=header.len, player).parse_next(input)?;
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
        assert_eq!(player.first_name, "J");
        assert_eq!(player.last_name, "Aagaard");
        assert_eq!(player.games, 5);
        assert_eq!(player.first_game, 3056);
    }
}
