use core::num::NonZeroU32;

use winnow::binary::bits::{self, Bits as BitInput};

use crate::{
    Player, Side,
    board::{Piece, Role},
    formats::{ByteInput as Input, prelude::*},
    position::SideTable,
    square::{File, Rank, Square},
};

use super::{Castles, Game, Header, Info, Mode, Move, PawnMove, Pieces, Position, Token};

pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    let first_game = be_u16.parse_next(input)?;
    let len = be_u32.parse_next(input)?;
    // Accumulated change in game-record byte lengths.
    let accum = be_u32.parse_next(input)?;
    let zero1 = be_u32.parse_next(input)?;
    let len2 = be_u32.parse_next(input)?;
    let zero2 = be_u32.parse_next(input)?;
    let accum2 = be_u32.parse_next(input)?;

    debug_assert_eq!(len, len2);
    debug_assert_eq!(accum, accum2);
    debug_assert_eq!(zero1, 0);
    debug_assert_eq!(zero2, 0);

    Ok(Header { first_game, len })
}

pub fn game_with_info(input: &mut Input<'_>) -> ModalResult<(Info, Game)> {
    let info = info.parse_next(input)?;
    let game = take(info.len).and_then(game(info)).parse_next(input)?;
    Ok((info, game))
}

pub fn game(info: Info) -> impl FnMut(&mut Input<'_>) -> ModalResult<Game> {
    let mode = info.mode;
    move |input: &mut Input<'_>| {
        if mode.is_undecodable() || mode.has_special_encoding() {
            return Err(backtrack());
        }

        let position = if mode.has_setup() {
            let position = position.parse_next(input)?;
            if mode.is_freestyle() {
                let _: (u32, u32) = (be_u32, be_u32).parse_next(input)?;
            }
            position
        } else {
            Position::standard()
        };
        let tokens = tokens.parse_next(input)?;
        Ok(Game { position, tokens })
    }
}

pub fn info(input: &mut Input<'_>) -> ModalResult<Info> {
    let mode = mode.parse_next(input)?;
    let len = be_u24.verify(|len| *len >= 4).parse_next(input)? as usize - 4;
    Ok(Info { len, mode })
}

pub fn mode(input: &mut Input<'_>) -> ModalResult<Mode> {
    u8.map(Mode).parse_next(input)
}

pub fn position(input: &mut Input<'_>) -> ModalResult<Position> {
    bits::bits::<_, _, ContextMode, _, _>(position_bits).parse_next(input)
}

fn position_bits(input: &mut BitInput<Input<'_>>) -> ModalResult<Position> {
    let _: u16 = bits::take(11usize).parse_next(input)?;
    let turn = turn.parse_next(input)?;
    let en_passant = en_passant.parse_next(input)?;
    let _: u8 = bits::take(4usize).parse_next(input)?;
    let castles = castles.parse_next(input)?;
    let round = round.parse_next(input)?;
    let (pieces, count) = pieces.parse_next(input)?;
    let padding = 128usize.checked_sub(4 * count).ok_or_else(backtrack)?;
    let _: u128 = bits::take(padding).parse_next(input)?;

    Ok(Position { pieces, turn, castles, en_passant, round })
}

fn turn(input: &mut BitInput<Input<'_>>) -> ModalResult<Player> {
    bits::bool.map(|black| if black { Player::Black } else { Player::White }).parse_next(input)
}

fn en_passant(input: &mut BitInput<Input<'_>>) -> ModalResult<Option<File>> {
    bits::take(4usize)
        .verify_map(|file: u8| match file {
            0 => Some(None),
            1..=8 => Some(File::from_index(file - 1)),
            _ => None,
        })
        .parse_next(input)
}

fn castles(input: &mut BitInput<Input<'_>>) -> ModalResult<Castles> {
    use Player::*;
    use Side::*;

    let mut castles = Castles::empty();
    for player in [Black, White] {
        for side in [King, Queen] {
            castles[player][side] = bits::bool.parse_next(input)?;
        }
    }
    Ok(castles)
}

fn round(input: &mut BitInput<Input<'_>>) -> ModalResult<NonZeroU32> {
    bits::take(8usize)
        // normalize round 0 to round 1
        .map(|round: u8| NonZeroU32::new(u32::from(round).max(1)).unwrap())
        .parse_next(input)
}

fn pieces(input: &mut BitInput<Input<'_>>) -> ModalResult<(Pieces, usize)> {
    let mut pieces = Pieces::empty();
    let mut count = 0;
    for index in 0..64 {
        let Some(piece) = piece.parse_next(input)? else { continue };
        let identity = pieces[piece.player][piece.role]
            .iter_mut()
            .find(|square| square.is_none())
            .ok_or_else(backtrack)?;
        *identity = Some(square(index));
        count += 1;
    }
    Ok((pieces, count))
}

fn piece(input: &mut BitInput<Input<'_>>) -> ModalResult<Option<Piece>> {
    if !bits::bool.parse_next(input)? {
        return Ok(None);
    }
    bits::take(4usize).verify_map(encoded_piece).map(Some).parse_next(input)
}

pub fn tokens(input: &mut Input<'_>) -> ModalResult<Vec<Token>> {
    let mut unscrambler = Unscrambler::default();
    repeat(0.., token(&mut unscrambler)).parse_next(input)
}

impl Position {
    fn standard() -> Self {
        use Player::*;

        Self {
            pieces: Pieces::standard(),
            turn: White,
            castles: Castles::filled(SideTable::filled(true)),
            en_passant: None,
            round: NonZeroU32::MIN,
        }
    }
}

fn encoded_piece(piece: u8) -> Option<Piece> {
    use Player::*;

    Some(match piece {
        1 => White.king(),
        2 => White.queen(),
        3 => White.knight(),
        4 => White.bishop(),
        5 => White.rook(),
        6 => White.pawn(),
        9 => Black.king(),
        10 => Black.queen(),
        11 => Black.knight(),
        12 => Black.bishop(),
        13 => Black.rook(),
        14 => Black.pawn(),
        _ => return None,
    })
}

#[derive(Clone, Copy, Default)]
pub struct Unscrambler {
    offset: u8,
}

impl Unscrambler {
    pub fn byte(self, input: &mut Input<'_>) -> ModalResult<u8> {
        u8.map(|byte| {
            let index = byte.wrapping_sub(self.offset) as usize;
            PERMUTE[index]
        })
        .parse_next(input)
    }

    pub fn bump(&mut self) {
        self.offset = self.offset.wrapping_add(1);
    }
}

fn token<'i, 'u>(
    unscrambler: &'u mut Unscrambler,
) -> impl FnMut(&mut Input<'i>) -> ModalResult<Token> + 'u {
    move |input| {
        let token = scrambled_token(*unscrambler).parse_next(input)?;
        if matches!(token, Token::Move(_)) {
            unscrambler.bump();
        }
        Ok(token)
    }
}

fn scrambled_token<'a>(
    unscrambler: Unscrambler,
) -> impl FnMut(&mut Input<'a>) -> ModalResult<Token> {
    use Move::{Castle, King, Null};

    move |input: &mut Input<'_>| {
        Ok(Token::Move(match unscrambler.byte(input)? {
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
            queen @ 0x0b..=0x26 => Move::queen(0, queen - 0x0b),
            queen @ 0x8f..=0xaa => Move::queen(1, queen - 0x8f),
            queen @ 0xab..=0xc6 => Move::queen(2, queen - 0xab),

            // Rook
            rook @ 0x27..=0x34 => Move::rook(0, rook - 0x27),
            rook @ 0x35..=0x42 => Move::rook(1, rook - 0x35),
            rook @ 0xc7..=0xd4 => Move::rook(2, rook - 0xc7),

            // Bishop
            bishop @ 0x43..=0x50 => Move::bishop(0, bishop - 0x43),
            bishop @ 0x51..=0x5e => Move::bishop(1, bishop - 0x51),
            bishop @ 0xd5..=0xe2 => Move::bishop(2, bishop - 0xd5),

            // Knight
            knight @ 0x5f..=0x66 => Move::knight(0, knight - 0x5f),
            knight @ 0x67..=0x6e => Move::knight(1, knight - 0x67),
            knight @ 0xe3..=0xea => Move::knight(2, knight - 0xe3),

            // Pawn
            pawn @ 0x6f..=0x8e => Move::pawn(pawn - 0x6f),

            // Promote
            0xeb => {
                let hi = u16::from(unscrambler.byte(input)?);
                let lo = u16::from(unscrambler.byte(input)?);
                Move::promote((hi << 8) | lo)
            }

            // Skip
            0xec..=0xfd => return Ok(Token::Skip),

            // Push
            0xfe => return Ok(Token::Push),

            // Pop
            0xff => return Ok(Token::Pop),
        }))
    }
}

fn square(index: u8) -> Square {
    debug_assert!(index < 64);
    Square::new(File::panicky_from_index(index >> 3), Rank::panicky_from_index(index & 7))
}

impl Move {
    fn bishop(i: u8, bishop: u8) -> Self {
        use Move::Bishop;

        match bishop {
            d @ 0x0..=0x6 => Bishop(i, d + 1, d + 1), // (1, 1) ... (7, 7)
            e @ 0x7..=0xd => Bishop(i, e - 6, 14 - e), // (1, 7) ... (7, 1)
            _ => unreachable!(),
        }
    }

    fn knight(i: u8, knight: u8) -> Self {
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

    fn pawn(pawn: u8) -> Self {
        use Move::Pawn;
        use PawnMove::*;

        let file = File::panicky_from_index(pawn / 4);
        match pawn % 4 {
            0 => Pawn(file, One),
            1 => Pawn(file, Two),
            2 => Pawn(file, Right),
            3 => Pawn(file, Left),
            _ => unreachable!(),
        }
    }

    fn promote(word: u16) -> Self {
        use Role::*;

        let from = word & 63;
        let to = (word >> 6) & 63;
        let role = match (word >> 12) & 3 {
            0 => Queen,
            1 => Rook,
            2 => Bishop,
            3 => Knight,
            _ => unreachable!(),
        };
        Self::FromTo(square(from as u8), square(to as u8), role)
    }

    fn queen(i: u8, queen: u8) -> Self {
        use Move::Queen;

        match queen {
            y @ 0x00..=0x06 => Queen(i, 0, y + 1), // (0, 1) ... (0, 7)
            x @ 0x07..=0x0d => Queen(i, x - 6, 0), // (1, 0) ... (7, 0)
            d @ 0x0e..=0x14 => Queen(i, d - 0x0d, d - 0x0d), // (1, 1) ... (7, 7)
            e @ 0x15..=0x1b => Queen(i, e - 0x14, 0x1c - e), // (1, 7) ... (7, 1)
            _ => unreachable!(),
        }
    }

    fn rook(i: u8, rook: u8) -> Self {
        use Move::Rook;

        match rook {
            x @ 0x0..=0x6 => Rook(i, 0, x + 1), // (0, 1) .. (0, 7)
            y @ 0x7..=0xd => Rook(i, y - 6, 0), // (1, 0) .. (7, 0)
            _ => unreachable!(),
        }
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::io::Cursor;

    use hex_literal::hex;
    use winnow::Parser as _;

    use crate::formats::{
        Fen,
        chessbase::{archive, index},
        pgn, twic,
    };

    use super::*;

    fn unpack(input: &[u8]) -> BTreeMap<String, Vec<u8>> {
        let mut input = input;
        let header = archive::read_header(&mut input).expect("can parse CBV header");
        header
            .members
            .iter()
            .map(|member| {
                let mut data = Vec::new();
                archive::unpack_to(member, &mut input, &mut data).expect("can unpack CBV member");
                assert_eq!(data.len(), member.len);
                (member.name.clone(), data)
            })
            .collect()
    }

    fn encode(tokens: &[u8]) -> Vec<u8> {
        let mut count = 0_u8;
        tokens
            .iter()
            .map(|token| {
                let index = PERMUTE
                    .iter()
                    .position(|candidate| candidate == token)
                    .expect("canonical token is represented") as u8;
                let encoded = index.wrapping_add(count);
                if *token <= 0xeb {
                    count = count.wrapping_add(1);
                }
                encoded
            })
            .collect()
    }

    fn mainline(game: &crate::Game) -> Vec<crate::Move> {
        let mut moves = Vec::new();
        let mut play = game.start_options().first();
        while let Some(current) = play {
            moves.push(current.play());
            play = current.options().first();
        }
        moves
    }

    #[test]
    #[ignore = "exhaustive comparison against the external TWIC fixture"]
    fn twic_games() {
        let index = twic::data(1616, "cbh");
        let index = index::index.parse(index.as_slice()).expect("can parse CBH file");

        let input = twic::data(1616, "cbg");
        let header = header.parse_next(&mut input.as_slice()).expect("can parse CBG header");
        assert_eq!(input.len(), header.len as usize);
        let pgns = pgn::stream::pgns(Cursor::new(twic::data(1616, "pgn")));

        let mut count = 0;
        for (i, (indexed, pgn)) in index.games.iter().zip(pgns).enumerate() {
            let mut input = &input[indexed.offsets.data..];
            let encoded = game_with_info.parse_next(&mut input).unwrap().1;
            let actual = crate::Game::from_chessbase(encoded)
                .unwrap_or_else(|error| panic!("CBG game {i}: {error}"));
            let expected = crate::Game::from_pgn(
                pgn.unwrap_or_else(|error| panic!("PGN game {i}: {error}"))
                    .unwrap_or_else(|error| panic!("PGN game {i}: {error}")),
            )
            .unwrap_or_else(|error| panic!("PGN game {i}: {error}"));

            assert_eq!(mainline(&actual), mainline(&expected), "game {i}");
            count += 1;
        }
        assert_eq!(count, index.games.len());
    }

    #[test]
    fn twic_game() {
        const INPUT: [u8; 109] = hex!(
            "0000006d0b08dc870210c19a0a110081
             82882659a72344d4124253447cf81804
             98bb07014e172835883cab0772e32431
             097526317d4384464d3e0a1490fc4c78
             fa61f88cb815f05b46c97dc02979f1e2
             462aafd3c9de2f9d06aa9914c7ffa818
             54e1b5db120fc36c1c9b9f2674"
        );

        let (_, encoded) = game_with_info.parse(INPUT.as_slice()).expect("can parse CBG game");
        let actual = crate::Game::from_chessbase(encoded).expect("can convert CBG game");
        let actual = mainline(&actual);
        let expected = crate::Game::from_pgn(
            pgn::pgn.parse("1. d4 Nf6 2. c4 e6 *").expect("valid expected PGN"),
        )
        .expect("valid expected game");

        assert_eq!(actual.len(), 104);
        assert!(actual.starts_with(&mainline(&expected)));
    }

    #[test]
    fn twic_freestyle_position() {
        const INPUT: [u8; 116] = hex!(
            "4a00007401000f019d83db9d83dbad83
             dd8d83d9a583dc9583daad83dda583dc
             181f30103717004bb9345889fd31341a
             aca950064f5218214585e02302590678
             e2f98f214964f66fbafa7cbc500affe0
             82feace6bb8c620a20d1db06775b6b9c
             8b44fd09fb752a58dd162a16e007df08
             2197d8a6"
        );

        let (info, encoded) =
            game_with_info.parse(INPUT.as_slice()).expect("can parse Freestyle CBG game");
        let position = crate::Position::try_from(encoded.position)
            .expect("can convert Freestyle CBG starting position");
        let expected: Fen = "nnrkbqrb/pppppppp/8/8/8/8/PPPPPPPP/NNRKBQRB w GCgc - 0 1"
            .parse()
            .expect("valid expected FEN");

        assert!(info.mode.has_setup());
        assert!(info.mode.is_freestyle());
        assert_eq!(position, expected.0);
    }

    #[test]
    fn variations() {
        let moves = encode(&[0xfe, 0x80, 0x80, 0xff, 0x7c, 0x7c, 0xff]);
        let len = u32::try_from(moves.len() + 4).unwrap();
        let mut input = vec![0, (len >> 16) as u8, (len >> 8) as u8, len as u8];
        input.extend(moves);
        let encoded = game_with_info.parse(input.as_slice()).unwrap().1;
        let actual = crate::Game::from_chessbase(encoded).unwrap();
        let expected =
            crate::Game::from_pgn(pgn::pgn.parse("1. e4 (1. d4 d5) 1... e5 *").unwrap()).unwrap();

        assert_eq!(pgn::Pgn::from(actual).movetext(), pgn::Pgn::from(expected).movetext(),);
    }

    #[test]
    #[ignore = "exhaustive Freestyle scan of the external TWIC fixture"]
    fn twic_freestyle() {
        let files = unpack(&twic::data(1632, "cbv"));
        let mut input = files["twic1632.cbg"].as_slice();
        header.parse_next(&mut input).expect("can parse CBG header");
        let pgns = pgn::stream::pgns(Cursor::new(twic::data(1632, "pgn")));

        let mut games = 0;
        let mut custom = 0;
        let mut freestyle = 0;
        // let mut lookup_tables = PlayerTable::filled([None; 256]);
        for (i, pgn) in pgns.enumerate() {
            let (info, encoded) = game_with_info.parse_next(&mut input).unwrap_or_else(|error| {
                panic!("can parse CBG game {games} with {} bytes remaining: {error}", input.len())
            });
            let expected = crate::Game::from_pgn(
                pgn.unwrap_or_else(|error| panic!("PGN game {i}: {error}"))
                    .unwrap_or_else(|error| panic!("PGN game {i}: {error}")),
            )
            .unwrap_or_else(|error| panic!("PGN game {i}: {error}"));
            if info.mode.has_setup() {
                let position = crate::Position::try_from(encoded.position.clone())
                    .unwrap_or_else(|error| panic!("CBG starting position of game {i}: {error}"));
                assert_eq!(position, expected.start(), "starting position of game {i}");
                // if i == 0 {
                //     score_row_seeds(encoded, &lookup, &expected);
                //     let lookup_conflicts =
                //         learn_lookup(&mut lookup_tables, encoded, lookup, &expected, i);
                //     eprintln!("side-specific lookup conflicts: {lookup_conflicts}");
                // }
            }
            games += 1;
            custom += usize::from(info.mode.has_setup());
            freestyle += usize::from(info.mode.is_freestyle());
        }

        assert!(input.is_empty());
        assert_eq!(games, 5_350);
        assert_eq!(custom, 61);
        assert_eq!(freestyle, 61);
    }

    /*
    Freestyle move-decoding experiments, disabled after the initial investigation.

    The 36-byte setup decoder reconstructs all 61 Freestyle starting positions in
    TWIC 1632, including the final big-endian Scharnagl ID. The ordinary CBG
    `raw - ply` permutation nevertheless fails from the first move of every game.

    For the first game (Scharnagl 75), g2-g4 transforms to 0xd6. Under the normal
    table this is in the third-bishop row, while its within-row offset would fit a
    pawn double move if the row beginning at 0xd5 had been reassigned to that pawn.
    This motivated testing a setup-dependent permutation of the normal move rows.

    A fixed row assignment was not observed: inferred bases changed 48 times for
    repeated piece identities. Separate lookup tables for White and Black still
    produced eight conflicting mappings. Adding or subtracting the Scharnagl ID
    inside `PERMUTE` also produced 48 row changes; adding it outside only shifts
    every inferred base and cannot improve consistency. Exhaustively trying all
    256 additive seeds inside `PERMUTE` found a best score of 45 changes (seed 60),
    which is not evidence of a simple offset.

    The remaining plausible hypothesis is a stateful move mapping derived from the
    current position or piece ordering, rather than a one-time permutation derived
    solely from the initial setup.

    fn score_row_seeds(encoded: Game<'_>, lookup: &Lookup, expected: &crate::Game) {
        let moves = mainline(expected);
        let score = |seed| {
            let mut lookup = lookup.clone();
            let mut bases = BTreeMap::new();
            let mut changes = 0;

            for (ply, (raw, play)) in encoded.tokens.iter().zip(&moves).enumerate() {
                let player = lookup.position.turn();
                let number = lookup.pieces[player][play.role]
                    .iter()
                    .position(|square| *square == Some(play.from))
                    .expect("moving piece has an identity") as u8;
                let canonical = canonical(&lookup, *play).expect("move has a compact encoding");
                let normal_base = canonical_base(play.role, number).expect("piece has a row");
                let transformed =
                    PERMUTE[raw.wrapping_sub(ply as u8).wrapping_add(seed) as usize];
                let base = transformed.wrapping_sub(canonical - normal_base);
                changes += usize::from(
                    bases
                        .insert((player, play.role, number), base)
                        .is_some_and(|previous| previous != base),
                );
                lookup.apply(*play);
            }

            changes
        };

        let scharnagl = u16::from_be_bytes([encoded.start.unwrap()[34], encoded.start.unwrap()[35]]);
        let id = scharnagl as u8;
        let mut scores: Vec<_> = (u8::MIN..=u8::MAX).map(|seed| (score(seed), seed)).collect();
        scores.sort_unstable();
        eprintln!(
            "row seed scores: zero={}, +id={}, -id={}, best={:?}",
            score(0),
            score(id),
            score(id.wrapping_neg()),
            &scores[..8],
        );
    }

    fn learn_lookup(
        tables: &mut PlayerTable<[Option<u8>; 256]>,
        encoded: Game<'_>,
        mut lookup: Lookup,
        expected: &crate::Game,
        game: usize,
    ) -> usize {
        let moves = mainline(expected);
        assert_eq!(encoded.tokens.len(), moves.len() + 1, "encoded length of game {game}");
        let mut conflicts = 0;
        let mut bases = BTreeMap::new();

        for (ply, (raw, play)) in encoded.tokens.iter().zip(moves).enumerate() {
            let player = lookup.position.turn();
            let number = lookup.pieces[player][play.role]
                .iter()
                .position(|square| *square == Some(play.from))
                .expect("moving piece has an identity") as u8;
            let canonical = canonical(&lookup, play)
                .unwrap_or_else(|| panic!("non-compact move in game {game}, ply {ply}: {play:?}"));
            let encoded = raw.wrapping_sub(ply as u8);
            let transformed = PERMUTE[encoded as usize];
            if let Some(base) = canonical_base(play.role, number) {
                let base = transformed.wrapping_sub(canonical - base);
                if let Some(previous) = bases.insert((player, play.role, number), base)
                    && previous != base
                {
                    eprintln!(
                        "base changed at ply {ply}: {player} {:?} {number}, {previous:02x} -> {base:02x}",
                        play.role,
                    );
                }
            }
            conflicts += usize::from(!learn(&mut tables[player], encoded, canonical));
            lookup.apply(play);
        }
        conflicts
    }

    fn canonical_base(role: Role, number: u8) -> Option<u8> {
        match role {
            Role::King => Some(0x01),
            Role::Queen => [0x0b, 0x8f, 0xab].get(number as usize).copied(),
            Role::Rook => [0x27, 0x35, 0xc7].get(number as usize).copied(),
            Role::Bishop => [0x43, 0x51, 0xd5].get(number as usize).copied(),
            Role::Knight => [0x5f, 0x67, 0xe3].get(number as usize).copied(),
            Role::Pawn => Some(0x6f + 4 * number),
        }
    }

    fn learn(table: &mut [Option<u8>; 256], encoded: u8, canonical: u8) -> bool {
        if let Some(previous) = table[encoded as usize] {
            previous == canonical
        } else {
            table[encoded as usize] = Some(canonical);
            true
        }
    }

    fn canonical(lookup: &Lookup, play: crate::Move) -> Option<u8> {
        if let Some(file) = play.kind.castle_rook_file() {
            return Some(if Side::of_rook(play.from, file) == Side::King { 0x09 } else { 0x0a });
        }

        let player = lookup.position.turn();
        let number = lookup.pieces[player][play.role]
            .iter()
            .position(|square| *square == Some(play.from))? as u8;
        let file = play.to.file() as i8 - play.from.file() as i8;
        let rank = play.to.rank() as i8 - play.from.rank() as i8;

        match play.role {
            Role::King => Some(match (file, rank) {
                (0, 1) => 0x01,
                (1, 1) => 0x02,
                (1, 0) => 0x03,
                (1, -1) => 0x04,
                (0, -1) => 0x05,
                (-1, -1) => 0x06,
                (-1, 0) => 0x07,
                (-1, 1) => 0x08,
                _ => return None,
            }),
            Role::Queen => {
                let base = [0x0b, 0x8f, 0xab].get(number as usize)?;
                Some(base + queen_offset(file, rank)?)
            }
            Role::Rook => {
                let base = [0x27, 0x35, 0xc7].get(number as usize)?;
                Some(base + rook_offset(file, rank)?)
            }
            Role::Bishop => {
                let base = [0x43, 0x51, 0xd5].get(number as usize)?;
                Some(base + bishop_offset(file, rank)?)
            }
            Role::Knight => {
                let base = [0x5f, 0x67, 0xe3].get(number as usize)?;
                let offset = match (file, rank) {
                    (2, 1) => 0,
                    (1, 2) => 1,
                    (-1, 2) => 2,
                    (-2, 1) => 3,
                    (-2, -1) => 4,
                    (-1, -2) => 5,
                    (1, -2) => 6,
                    (2, -1) => 7,
                    _ => return None,
                };
                Some(base + offset)
            }
            Role::Pawn if play.kind.is_promote() => None,
            Role::Pawn => {
                let forward = if player.is_white() { 1 } else { -1 };
                let movement = match (file, rank) {
                    (0, rank) if rank == forward => 0,
                    (0, rank) if rank == 2 * forward => 1,
                    (file, rank) if file == forward && rank == forward => 2,
                    (file, rank) if file == -forward && rank == forward => 3,
                    _ => return None,
                };
                Some(0x6f + 4 * number + movement)
            }
        }
    }

    fn queen_offset(file: i8, rank: i8) -> Option<u8> {
        match (file.rem_euclid(8) as u8, rank.rem_euclid(8) as u8) {
            (0, rank @ 1..=7) => Some(rank - 1),
            (file @ 1..=7, 0) => Some(6 + file),
            (file @ 1..=7, rank) if file == rank => Some(13 + file),
            (file @ 1..=7, rank) if file + rank == 8 => Some(20 + file),
            _ => None,
        }
    }

    fn rook_offset(file: i8, rank: i8) -> Option<u8> {
        match (file.rem_euclid(8) as u8, rank.rem_euclid(8) as u8) {
            (0, rank @ 1..=7) => Some(rank - 1),
            (file @ 1..=7, 0) => Some(6 + file),
            _ => None,
        }
    }

    fn bishop_offset(file: i8, rank: i8) -> Option<u8> {
        match (file.rem_euclid(8) as u8, rank.rem_euclid(8) as u8) {
            (file @ 1..=7, rank) if file == rank => Some(file - 1),
            (file @ 1..=7, rank) if file + rank == 8 => Some(6 + file),
            _ => None,
        }
    }
    */
}
