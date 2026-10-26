//! ChessBase annotation file format (CBA).

use crate::{
    formats::bytes_prelude::*,
    formats::prelude::Framed,
    game::{Arrow, Color, Marker},
    square::Square,
};

pub mod convert;

/// ChessBase annotation filename suffix.
pub const SUFFIX: &str = "cba";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Info {
    pub len: usize,
}

impl Info {
    pub const LEN: usize = 14;
}

impl Framed for Info {
    const LEN: usize = Self::LEN;

    fn total(&self) -> usize {
        self.len
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Annotations {
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    /// Global zero-based move encounter index, or `None` for the game root.
    pub index: Option<usize>,
    pub annotation: Annotation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Annotation {
    Before(Comment),
    After(Comment),
    Symbols(Vec<u8>),
    Squares(Vec<Marker>),
    Arrows(Vec<Arrow>),
    Unknown { kind: u8, data: Vec<u8> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Comment {
    pub language: u16,
    pub text: Vec<u8>,
}

pub fn info(input: &mut Input<'_>) -> ModalResult<Info> {
    preceded(take(10usize), be_u32)
        .verify_map(|len| usize::try_from(len).ok().filter(|len| *len >= Info::LEN))
        .map(|len| Info { len })
        .parse_next(input)
}

pub fn annotations(input: &mut Input<'_>) -> ModalResult<Annotations> {
    let mut index = move_index.parse_next(input)?;
    let mut entries = Vec::new();

    loop {
        let kind = u8.parse_next(input)?;
        let len = be_u16.verify_map(|len| len.checked_sub(6)).parse_next(input)? as usize;
        let annotation = take(len).and_then(annotation(kind)).parse_next(input)?;
        entries.push(Entry { index, annotation });

        if input.is_empty() {
            break;
        }
        index = move_index.parse_next(input)?;
    }

    Ok(Annotations { entries })
}

pub fn move_index(input: &mut Input<'_>) -> ModalResult<Option<usize>> {
    be_u24.map(|index| (index != 0xff_ffff).then_some(index as usize)).parse_next(input)
}

pub fn annotation(kind: u8) -> impl FnMut(&mut Input<'_>) -> ModalResult<Annotation> {
    move |input| {
        Ok(match kind {
            0x82 => Annotation::Before(comment.parse_next(input)?),
            0x02 => Annotation::After(comment.parse_next(input)?),
            0x03 => Annotation::Symbols(repeat(0..=3, u8).parse_next(input)?),
            0x04 => Annotation::Squares(repeat(0.., marker).parse_next(input)?),
            0x05 => Annotation::Arrows(repeat(0.., arrow).parse_next(input)?),
            _ => {
                let data = input.to_vec();
                *input = &[];
                Annotation::Unknown { kind, data }
            }
        })
    }
}

pub fn comment(input: &mut Input<'_>) -> ModalResult<Comment> {
    let language = be_u16.parse_next(input)?;
    let text = input.to_vec();
    *input = &[];
    Ok(Comment { language, text })
}

pub fn marker(input: &mut Input<'_>) -> ModalResult<Marker> {
    let color = color.parse_next(input)?;
    let square = square.parse_next(input)?;
    Ok(Marker { color, square })
}

pub fn arrow(input: &mut Input<'_>) -> ModalResult<Arrow> {
    let color = color.parse_next(input)?;
    let from = square.parse_next(input)?;
    let to = square.parse_next(input)?;
    Ok(Arrow { color, from, to })
}

pub fn color(input: &mut Input<'_>) -> ModalResult<Option<Color>> {
    u8.map(|color| match color {
        2 => Some(Color::Green),
        3 => Some(Color::Yellow),
        4 => Some(Color::Red),
        _ => None,
    })
    .parse_next(input)
}

pub fn square(input: &mut Input<'_>) -> ModalResult<Square> {
    u8.verify_map(|square| {
        let square = square.checked_sub(1)?;
        Square::from_index(square)
    })
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use super::*;

    #[test]
    fn root_comment() {
        const INPUT: [u8; 15] = hex!("ffffff02000f000032302f35343420");

        let annotations = super::annotations.parse(INPUT.as_slice()).unwrap();
        assert_eq!(
            annotations.entries,
            vec![Entry {
                index: None,
                annotation: Annotation::After(Comment { language: 0, text: b"20/544 ".to_vec() }),
            }]
        );
    }

    #[test]
    fn symbols() {
        const INPUT: [u8; 14] = hex!("0000140300070200001503000703");

        let annotations = super::annotations.parse(INPUT.as_slice()).unwrap();
        assert_eq!(annotations.entries.len(), 2);
        assert_eq!(annotations.entries[0].index, Some(20));
        assert_eq!(annotations.entries[0].annotation, Annotation::Symbols(vec![2]));
        assert_eq!(annotations.entries[1].index, Some(21));
        assert_eq!(annotations.entries[1].annotation, Annotation::Symbols(vec![3]));
    }
}
