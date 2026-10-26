//! ChessBase source file format.

use crate::formats::bytes_prelude::*;

use super::{
    common::{Header, Node, header, node},
    index::{Date, le_date},
    text,
};

/// ChessBase source filename suffix.
pub const SUFFIX: &str = "cbs";

#[derive(Clone, Debug)]
pub struct Source {
    pub deleted: bool,
    pub title: Option<Text>,
    pub publisher: Option<Text>,
    pub publication_date: Date,
    pub version: u8,
    pub date: Date,
    pub quality: u8,
}

pub fn source(input: &mut Input<'_>) -> ModalResult<Source> {
    let Node { deleted } = node.parse_next(input)?;
    let title = take(25u8).map(text).parse_next(input)?;
    let publisher = take(16u8).map(text).parse_next(input)?;
    let publication_date = le_date.parse_next(input)?;
    let version = u8.parse_next(input)?;
    let date = le_date.parse_next(input)?;
    take(4u8).parse_next(input)?;
    let quality = u8.parse_next(input)?;
    take(6u8).parse_next(input)?;

    Ok(Source { deleted, title, publisher, publication_date, version, date, quality })
}

#[derive(Clone, Debug)]
pub struct Sources {
    pub header: Header,
    pub sources: Vec<Source>,
}

pub fn sources(input: &mut Input<'_>) -> ModalResult<Sources> {
    let header = header.verify(|header| header.record_len == 59).parse_next(input)?;
    let sources = repeat(header.len, source).parse_next(input)?;
    Ok(Sources { header, sources })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use hex_literal::hex;

    use crate::formats::twic;

    use super::*;

    #[test]
    fn source() {
        const INPUT: [u8; 68] = hex!(
            "ffffffffffffffff0054686520576565
             6b20696e204368657373203136313600
             00004d61726b2043726f777468657200
             00005bd30f005bd30f0001026b180000
             ffffffff"
        );
        let date = Date { year: Some(2025), month: Some(10), day: Some(27) };

        let source = super::source.parse(INPUT.as_slice()).expect("can parse CBS source");
        assert!(!source.deleted);
        assert_eq!(source.title.as_deref(), Some("The Week in Chess 1616"));
        assert_eq!(source.publisher.as_deref(), Some("Mark Crowther"));
        assert_eq!(source.publication_date, date);
        assert_eq!(source.version, 0);
        assert_eq!(source.date, date);
        assert_eq!(source.quality, 24);
    }

    #[test]
    #[ignore = "exploratory output from the external TWIC fixture"]
    fn twic_sources() {
        let input = twic::data(1616, "cbs");
        let sources = sources.parse(input.as_slice()).map_err(drop).expect("can parse CBS file");
        println!("{:?}", sources.header);
        for source in &sources.sources {
            println!("{source:?}");
        }
    }

    #[test]
    #[ignore = "exploratory output from external book fixtures"]
    fn book_sources() {
        for name in ["Best Lessons.cbs", "magictal.cbs"] {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("twic").join(name);
            let input = fs::read(path).expect("can read CBS fixture");
            let sources =
                sources.parse(input.as_slice()).map_err(drop).expect("can parse CBS file");
            println!("{name}");
            for source in sources.sources.iter().filter(|source| !source.deleted) {
                println!("{source:?}");
            }
        }
    }
}
