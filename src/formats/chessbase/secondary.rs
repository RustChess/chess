//! ChessBase secondary game header format.

use crate::formats::bytes_prelude::*;

/// ChessBase secondary game header filename suffix.
pub const SUFFIX: &str = "cbj";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub version: u32,
    pub record_len: usize,
    pub len: usize,
}

impl Header {
    pub const LEN: usize = 32;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Secondary {
    pub white_team: Option<usize>,
    pub black_team: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Secondaries {
    pub header: Header,
    pub secondaries: Vec<Secondary>,
}

pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    let version = le_u32.parse_next(input)?;
    let record_len = le_u32.verify(|len| *len >= 8).map(|len| len as usize).parse_next(input)?;
    let len = le_u32.map(|len| len as usize).parse_next(input)?;
    take(Header::LEN - 12).parse_next(input)?;
    Ok(Header { version, record_len, len })
}

pub fn secondary(record_len: usize) -> impl FnMut(&mut Input<'_>) -> ModalResult<Secondary> {
    move |input| {
        let white_team = be_u32.map(team).parse_next(input)?;
        let black_team = be_u32.map(team).parse_next(input)?;
        take(record_len - 8).parse_next(input)?;
        Ok(Secondary { white_team, black_team })
    }
}

pub fn secondaries(input: &mut Input<'_>) -> ModalResult<Secondaries> {
    let header = header.parse_next(input)?;
    let secondaries = repeat(header.len, secondary(header.record_len)).parse_next(input)?;
    Ok(Secondaries { header, secondaries })
}

fn team(index: u32) -> Option<usize> {
    (index != u32::MAX).then_some(index as usize)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use hex_literal::hex;

    use crate::formats::twic;

    use super::*;

    #[test]
    fn modern() {
        const INPUT: [u8; 120] = hex!(
            "00000053000000060000000000000000
             00000000000000000000000000000000
             00000000001a00000000000000000000
             00000000000000000000000000000000
             00000000000000000000000000000002
             0007f3376930e0510000000000000000
             00000000000000000000000001f0b393
             cba3054800000000"
        );

        let secondary = super::secondary(INPUT.len())
            .parse(INPUT.as_slice())
            .expect("can parse modern CBJ record");
        assert_eq!(secondary.white_team, Some(83));
        assert_eq!(secondary.black_team, Some(6));
    }

    #[test]
    #[ignore = "exploratory parsing of external fixtures"]
    fn fixtures() {
        let input = twic::data(1616, "cbj");
        let twic =
            secondaries.parse(input.as_slice()).map_err(drop).expect("can parse TWIC CBJ file");
        assert_eq!(twic.header, Header { version: 11, record_len: 120, len: 6251 });
        assert_eq!(twic.secondaries[0].white_team, Some(83));
        assert_eq!(twic.secondaries[0].black_team, Some(6));

        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("twic/Best Lessons.cbj");
        let input = fs::read(path).expect("can read CBJ fixture");
        let secondaries =
            secondaries.parse(input.as_slice()).map_err(drop).expect("can parse legacy CBJ file");
        assert_eq!(secondaries.header, Header { version: 1, record_len: 8, len: 53 });
    }
}
