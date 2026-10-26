//! ChessBase annotator file format.

use crate::formats::bytes_prelude::*;

use super::{
    common::{Header, Node, header, node},
    text,
};

/// ChessBase annotator filename suffix.
pub const SUFFIX: &str = "cbc";

#[derive(Clone, Debug)]
pub struct Annotator {
    pub deleted: bool,
    pub name: Option<Text>,
    pub games: usize,
    pub first_game: usize,
}

pub fn annotator(input: &mut Input<'_>) -> ModalResult<Annotator> {
    let Node { deleted } = node.parse_next(input)?;
    let name = take(45u8).map(text).parse_next(input)?;
    let games = le_u32.parse_next(input)? as usize;
    let first_game = le_u32.parse_next(input)? as usize;

    Ok(Annotator { deleted, name, games, first_game })
}

#[derive(Clone, Debug)]
pub struct Annotators {
    pub header: Header,
    pub annotators: Vec<Annotator>,
}

pub fn annotators(input: &mut Input<'_>) -> ModalResult<Annotators> {
    let header = header.verify(|header| header.record_len == 53).parse_next(input)?;
    let annotators = repeat(header.len, annotator).parse_next(input)?;
    Ok(Annotators { header, annotators })
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use super::*;

    #[test]
    fn annotator() {
        const INPUT: [u8; 62] = hex!(
            "ffffffffffffffff0053636865706c65
             722c4161726f6e000000000000000000
             00000000000000000000000000000000
             0000000000000100000032000000"
        );

        let annotator = super::annotator.parse(INPUT.as_slice()).unwrap();
        assert!(!annotator.deleted);
        assert_eq!(annotator.name.as_deref(), Some("Schepler,Aaron"));
        assert_eq!(annotator.games, 1);
        assert_eq!(annotator.first_game, 50);
    }
}
