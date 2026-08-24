//! Chessbase player file format

use super::{ByteInput as Input, cbv::cstr, prelude::*};

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub len: usize,
    pub avl_root: u32,
    pub first_deleted: Option<usize>,
    pub record_len: usize,
    pub len_existing: usize,
}

pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    let len = le_u32.parse_next(input)? as usize;
    let avl_root = le_u32.parse_next(input)?;
    let one_to_zero = le_u32.parse_next(input)?;
    assert_eq!(one_to_zero, 1234567890);
    let record_len = le_u32.parse_next(input)? as usize;
    // Not true for CBT which reuses the header
    // println!("size: {size}");
    // assert_eq!(67, size + 9);
    let first_deleted = le_i32.map(|x| (x != -1).then_some(x as usize)).parse_next(input)?;
    let len_existing = le_u32.parse_next(input)? as usize;
    // assert!(len_existing >= len);
    // padding? is 4 in an example
    let more_offset = le_u32.parse_next(input)?;
    assert!(more_offset == 0 || more_offset == 4);
    // println!("more offset: {more_offset}");
    // compared to talkchess.com post, there's another four bytes
    let padding = take(more_offset).parse_next(input)?;
    assert!(padding.iter().all(|x| *x == 0));

    Ok(Header { len, avl_root, record_len, first_deleted, len_existing })
}

// numbers where -1 mean missing or unknown
// fn optional1<T: Default + PartialEq>(x: T) -> Option<T> {
//     (x != T::default()).then_some(x)
// }
fn optional1(x: i32) -> Option<usize> {
    assert!(x >= -1);
    if x == -1 { None } else { Some(x as usize) }
}

#[derive(Clone, Debug)]
pub struct Record {
    pub deleted: bool,
    pub first_name: String,
    pub last_name: String,
    pub games: usize,
    pub first_game: usize,
}

pub fn record(input: &mut Input<'_>) -> ModalResult<Record> {
    let (left_child, deleted) = le_i32
        .map(|x| if x == -999 { (None, true) } else { (optional1(x), false) })
        .parse_next(input)?;
    let (right_child, next_deleted) = le_i32
        .map(|x| if deleted { (None, optional1(x)) } else { (optional1(x), None) })
        .parse_next(input)?;
    let right_height_minus_left_height = i8.parse_next(input)?;
    if deleted {
        assert_eq!(right_height_minus_left_height, 0)
    };
    let last_name = take(30u8).map(cstr).parse_next(input)?;
    let first_name = take(20u8).map(cstr).parse_next(input)?;
    let games = le_u32.parse_next(input)? as usize;
    let first_game = le_u32.parse_next(input)? as usize;

    let _ = (left_child, right_child, next_deleted);

    Ok(Record { deleted, first_name, last_name, games, first_game })
}

#[derive(Clone, Debug)]
pub struct Players {
    pub header: Header,
    pub records: Vec<Record>,
}

pub fn players(input: &mut Input<'_>) -> ModalResult<Players> {
    let header = header.parse_next(input)?;
    let records: Vec<Record> = repeat(..=header.len, record).parse_next(input)?;
    Ok(Players { header, records })
}

#[test]
fn example() {
    let input = include_bytes!("../../examples/twic1616.cbp");
    let header = header.parse_next(&mut input.as_slice()).expect("can parse CBP header");
    println!("{header:?}");
    let players = players.parse(input.as_slice()).map_err(drop).expect("can parse CBP file");
    println!("{:?}", players.header);
    for player in players.records.iter().take(3) {
        println!("{player:?}");
    }
    for player in players.records.iter().rev().take(3) {
        println!("{player:?}");
    }
}
