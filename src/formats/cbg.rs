//! Chessbase game file format (CBG)

// Header: 26 B
// Records: variable length
//

use crate::{
    board::Role,
    square::{File, Square},
};

use super::{ByteInput as Input, prelude::*};

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub first_game: u16,
    pub len: u32,
    pub accum: u32,
}

// This seems wrong..
pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    let first_game = be_u16.parse_next(input)?;
    let len = be_u32.parse_next(input)?;
    let accum = be_u32.parse_next(input)?;
    let zero1 = be_u32.parse_next(input)?;
    let len2 = be_u32.parse_next(input)?;
    let zero2 = be_u32.parse_next(input)?;

    assert_eq!(len, len2);
    assert_eq!(zero1, 0);
    assert_eq!(zero2, 0);

    Ok(Header { first_game, len, accum })
}

#[derive(Clone, Copy, Debug)]
pub struct Info {
    pub len: u32,
    pub not_initial: bool,
    pub not_encoded: bool,
    pub special_encoding: bool,
    pub freestyle: bool,
}

pub fn info(input: &mut Input<'_>) -> ModalResult<Info> {
    let info = u8.parse_next(input)?;
    let len = be_u24.parse_next(input)?;
    let not_initial = info & 0x40 != 0;
    let not_encoded = info & 0x80 != 0;
    let special_encoding = info & 4 != 0;
    let freestyle = info & 0xA > 0;
    // assert_eq!(info, 0);
    Ok(Info { len, not_initial, not_encoded, special_encoding, freestyle })
}
#[derive(Clone, Copy, Debug)]
pub struct Game {
    pub info: Info,
}

pub fn game(input: &mut Input<'_>) -> ModalResult<Game> {
    let info = info.parse_next(input)?;
    if info.not_initial {
        unimplemented!();
    }
    // assert!(!info.special_encoding);
    // assert!(!info.freestyle);

    Ok(Game { info })
}

#[test]
fn example() {
    let input = include_bytes!("../../examples/twic1616.cbh");
    let headers = super::cbh::headers.parse(input).expect("can pass CBH file");

    let input = include_bytes!("../../examples/twic1616.cbg");
    let header = header.parse_next(&mut input.as_slice()).expect("can parse CBG header");
    assert_eq!(input.len(), header.len as usize);
    println!("{header:?}");

    for (i, record) in headers.records.iter().enumerate() {
        let mut input = &input[record.game_offset..];
        let game = game.parse_next(&mut input).unwrap();
        println!(":: {i}: {game:?}");
    }
    // panic!("wtf");
}

/// For some obfuscation reason, the (mostly) logical move-to-byte
/// encodings are permuted.
fn permute(x: u8, count: usize) -> u8 {
    let index = x.wrapping_sub(count as u8) as usize;
    PERMUTE[index]
}

/// The tokens that moves are encoded as
#[derive(Clone, Copy, Debug)]
pub enum Token {
    Pop,
    Push,
    Move(Move),
    Skip,
}

#[derive(Clone, Copy, Debug)]
pub enum Move {
    Null,
    // (x, y) moves: -1 <= x,y <= 1
    King(i8, i8),
    // x move; x = 2 or x = -2
    Castle(i8),
    // queen index 0, 1, or 2, and (x, y) moves: 1 <= x,y < 8
    Queen(u8, u8, u8),
    // rook index 0, 1, or 2, and (x, y) moves: 1 <= x,y < 8
    Rook(u8, u8, u8),
    // bishop index 0, 1, or 2, and (x, y) moves: 1 <= x,y < 8
    Bishop(u8, u8, u8),
    // knight index 0, 1, or 2, and (x, y) moves: -2 <= x,y < 2
    Knight(u8, i8, i8),
    // any file
    Pawn(File, PawnMove),
    // (from, to, role) squares and valid promotion Role
    //
    // It seems that "a fourth piece never promotes, and its movements
    // are always captured with multiple bytes".
    //
    // Only if the move is a pawn move that promotes, is the role valid.
    //
    // Any move could be encoded (UCI-style) with this (from, to) method,
    // but typically it's only done for moving "fourth pieces" or pawns.
    FromTo(Square, Square, Role),
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum PawnMove {
    One,
    Two,
    // captures, left/right from the perspective of the player
    Left,
    Right,
}

// compactification of scidb's cbh_decoder.cpp
pub fn token<'a>(count: &mut usize) -> impl FnMut(&mut Input<'a>) -> ModalResult<Token> {
    use Move::{Castle, King, Null, Pawn};

    move |input: &mut Input<'_>| {
        // seems like every game is supposed to end with this
        // but there are (rare) games that are missing the token,
        // so we patch it in.
        if input.is_empty() {
            return Ok(Token::Pop);
        }

        let byte = permute(u8.parse_next(input)?, *count);

        let play = match byte {
            // Null
            0x00 => Null,

            // King
            0x01 => King(0, 1),
            0x02 => King(1, 1),
            0x03 => King(1, 0),
            0x04 => King(1, -1),
            0x05 => King(0, -1),
            0x06 => King(-1, -1),
            0x07 => King(-1, 0),
            0x08 => King(-1, 1),
            0x09 => Castle(2),
            0x0a => Castle(-2),

            // Queen
            queen @ 0x0b..=0x26 => queen_move(0, queen - 0x0b),
            queen @ 0x8f..=0xaa => queen_move(1, queen - 0x8f),
            queen @ 0xab..=0xc6 => queen_move(2, queen - 0xab),

            // Rook
            rook @ 0x27..=0x34 => rook_move(0, rook - 0x27),
            rook @ 0x35..=0x42 => rook_move(1, rook - 0x35),
            rook @ 0xc7..=0xd4 => rook_move(2, rook - 0xc7),

            // Bishop
            bishop @ 0x43..=0x50 => bishop_move(0, bishop - 0x43),
            bishop @ 0x51..=0x5e => bishop_move(1, bishop - 0x51),
            bishop @ 0xd5..=0xe2 => bishop_move(2, bishop - 0xd5),

            // Knight
            knight @ 0x5f..=0x66 => knight_move(0, knight - 0x5f),
            knight @ 0x67..=0x6e => knight_move(1, knight - 0x67),
            knight @ 0xe3..=0xea => knight_move(2, knight - 0xea),

            // Pawn
            pawn @ 0x6f..=0x8e => {
                use PawnMove::*;

                let offset = pawn - 0x6f;
                let file = File::panicky_from_index(offset / 8);
                match offset % 4 {
                    0 => Pawn(file, One),
                    1 => Pawn(file, Two),
                    2 => Pawn(file, Right),
                    3 => Pawn(file, Left),
                    _ => unreachable!(),
                }
            }

            // Promote
            0xeb => {
                use Role::*;

                let hi = permute(u8.parse_next(input)?, *count) as u16;
                let lo = permute(u8.parse_next(input)?, *count) as u16;
                let word = (hi << 8) | lo;

                let from = word & 63;
                let to = (word >> 6) & 63;

                let promoted = match (word >> 12) & 3 {
                    0 => Queen,
                    1 => Rook,
                    2 => Bishop,
                    3 => Knight,
                    _ => unreachable!(),
                };

                Move::FromTo(
                    Square::panicky_from_index(from as u8),
                    Square::panicky_from_index(to as u8),
                    promoted,
                )
            }

            // Skip
            0xec..=0xfd => return Ok(Token::Skip),

            // Push
            0xfe => return Ok(Token::Push),

            // Pop
            0xff => return Ok(Token::Pop),
        };

        *count += 1;
        Ok(Token::Move(play))
    }
}

fn queen_move(i: u8, queen: u8) -> Move {
    use Move::Queen;

    match queen {
        y @ 0x00..=0x06 => Queen(i, 0, y + 1), // (0, 1) ... (0, 7)
        x @ 0x07..=0x14 => Queen(i, x - 6, 0), // (1, 0) ... (7, 0)
        d @ 0x15..=0x1b => Queen(i, d - 0x14, d - 0x14), // (1, 1) ... (7, 7)
        e @ 0x1c..=0x22 => Queen(i, e - 0x1b, 0x1c + 7 - e), // (1, 7) ... (7, 1)
        _ => unreachable!(),
    }
}

fn rook_move(i: u8, rook: u8) -> Move {
    use Move::Rook;

    match rook {
        x @ 0x0..=0x6 => Rook(i, 0, x + 1), // (0, 1) .. (0, 7)
        y @ 0x7..=0xd => Rook(i, y - 6, 0), // (1, 0) .. (7, 0)
        _ => unreachable!(),
    }
}

fn bishop_move(i: u8, bishop: u8) -> Move {
    use Move::Bishop;

    match bishop {
        d @ 0x0..=0x6 => Bishop(i, d + 1, d + 1), // (1, 1) ... (7, 7)
        e @ 0x7..=0xd => Bishop(i, e - 6, 14 - e), // (1, 7) ... (7, 1)
        _ => unreachable!(),
    }
}

fn knight_move(i: u8, knight: u8) -> Move {
    use Move::Knight;

    match knight {
        0 => Knight(i, 2, 1),
        1 => Knight(i, 1, 2),
        2 => Knight(i, -1, 2),
        3 => Knight(i, -2, 1),
        4 => Knight(i, -2, -1),
        5 => Knight(i, -1, -2),
        6 => Knight(i, 1, -2),
        7 => Knight(i, 2, -1),
        _ => unreachable!(),
    }
}

pub const PERMUTE: [u8; 256] = [
    0xa2, 0x95, 0x43, 0xf5, 0xc1, 0x3d, 0x4a, 0x6c, //   0 -   7
    0x53, 0x83, 0xcc, 0x7c, 0xff, 0xae, 0x68, 0xad, //   8 -  15
    0xd1, 0x92, 0x8b, 0x8d, 0x35, 0x81, 0x5e, 0x74, //  16 -  23
    0x26, 0x8e, 0xab, 0xca, 0xfd, 0x9a, 0xf3, 0xa0, //  24 -  31
    0xa5, 0x15, 0xfc, 0xb1, 0x1e, 0xed, 0x30, 0xea, //  32 -  39
    0x22, 0xeb, 0xa7, 0xcd, 0x4e, 0x6f, 0x2e, 0x24, //  40 -  47
    0x32, 0x94, 0x41, 0x8c, 0x6e, 0x58, 0x82, 0x50, //  48 -  55
    0xbb, 0x02, 0x8a, 0xd8, 0xfa, 0x60, 0xde, 0x52, //  56 -  63
    0xba, 0x46, 0xac, 0x29, 0x9d, 0xd7, 0xdf, 0x08, //  64 -  71
    0x21, 0x01, 0x66, 0xa3, 0xf1, 0x19, 0x27, 0xb5, //  72 -  79
    0x91, 0xd5, 0x42, 0x0e, 0xb4, 0x4c, 0xd9, 0x18, //  80 -  87
    0x5f, 0xbc, 0x25, 0xa6, 0x96, 0x04, 0x56, 0x6a, //  88 -  95
    0xaa, 0x33, 0x1c, 0x2b, 0x73, 0xf0, 0xdd, 0xa4, //  96 - 103
    0x37, 0xd3, 0xc5, 0x10, 0xbf, 0x5a, 0x23, 0x34, // 104 - 111
    0x75, 0x5b, 0xb8, 0x55, 0xd2, 0x6b, 0x09, 0x3a, // 112 - 119
    0x57, 0x12, 0xb3, 0x77, 0x48, 0x85, 0x9b, 0x0f, // 120 - 127
    0x9e, 0xc7, 0xc8, 0xa1, 0x7f, 0x7a, 0xc0, 0xbd, // 128 - 135
    0x31, 0x6d, 0xf6, 0x3e, 0xc3, 0x11, 0x71, 0xce, // 136 - 143
    0x7d, 0xda, 0xa8, 0x54, 0x90, 0x97, 0x1f, 0x44, // 144 - 151
    0x40, 0x16, 0xc9, 0xe3, 0x2c, 0xcb, 0x84, 0xec, // 152 - 159
    0x9f, 0x3f, 0x5c, 0xe6, 0x76, 0x0b, 0x3c, 0x20, // 160 - 167
    0xb7, 0x36, 0x00, 0xdc, 0xe7, 0xf9, 0x4f, 0xf7, // 168 - 175
    0xaf, 0x06, 0x07, 0xe0, 0x1a, 0x0a, 0xa9, 0x4b, // 176 - 183
    0x0c, 0xd6, 0x63, 0x87, 0x89, 0x1d, 0x13, 0x1b, // 184 - 191
    0xe4, 0x70, 0x05, 0x47, 0x67, 0x7b, 0x2f, 0xee, // 192 - 199
    0xe2, 0xe8, 0x98, 0x0d, 0xef, 0xcf, 0xc4, 0xf4, // 200 - 207
    0xfb, 0xb0, 0x17, 0x99, 0x64, 0xf2, 0xd4, 0x2a, // 208 - 215
    0x03, 0x4d, 0x78, 0xc6, 0xfe, 0x65, 0x86, 0x88, // 216 - 223
    0x79, 0x45, 0x3b, 0xe5, 0x49, 0x8f, 0x2d, 0xb9, // 224 - 231
    0xbe, 0x62, 0x93, 0x14, 0xe9, 0xd0, 0x38, 0x9c, // 232 - 239
    0xb2, 0xc2, 0x59, 0x5d, 0xb6, 0x72, 0x51, 0xf8, // 240 - 247
    0x28, 0x7e, 0x61, 0x39, 0xe1, 0xdb, 0x69, 0x80, // 248 - 255
];
