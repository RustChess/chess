use core::{error, fmt, str::FromStr};

pub use std::collections::BTreeSet as Set;

pub use winnow::ModalResult;
pub use winnow::Parser;

mod bits;

pub mod chessbase;
pub mod fen;
pub mod pgn;
pub mod san;
pub mod text;
pub mod uci;

#[doc(inline)]
pub use bits::Bits;

#[doc(inline)]
pub use fen::Fen;
impl StrFormat for Fen {}

#[doc(inline)]
pub use pgn::Pgn;
impl StrFormat for Pgn {}

#[doc(inline)]
pub use san::San;
impl StrFormat for San {}

#[doc(inline)]
pub use text::Text;
impl StrFormat for Text {}

/// Slice of bytes.
pub type ByteInput<'a> = &'a [u8];

/// Slice of UTF-8 text.
pub type StrInput<'a> = &'a str;

/// Complete text format with parsing, formatting, and format-specific errors.
pub trait StrFormat: fmt::Display + FromStr
where
    Self::Err: error::Error,
{
}

mod prelude {
    use std::io::{self, Read};

    use super::ByteInput as Input;

    pub use winnow::{
        ModalResult,
        Parser,
        ascii::{dec_uint, multispace0, multispace1, space0, space1},
        binary::{be_u16, be_u24, be_u32, i8, le_i32, le_u16, le_u24, le_u32, u8},
        combinator::{
            alt, backtrack_err, cut_err, delimited, opt, preceded, repeat, separated,
            separated_pair, seq, terminated,
        },
        error::{ContextError, ErrMode, StrContext},
        // stream::Bytes,
        token::{any, none_of, one_of, take, take_till, take_while},
    };

    pub type ContextMode = ErrMode<ContextError>;

    pub fn backtrack() -> ContextMode {
        ErrMode::Backtrack(ContextError::new())
    }

    pub trait Framed {
        const LEN: usize;

        fn total(&self) -> usize;
    }

    pub enum FrameError<E> {
        Error(E),
        Io(io::Error),
    }

    pub fn framed<Header, T, H, P, F, E>(
        input: &mut impl Read,
        data: &mut Vec<u8>,
        mut header: H,
        parser: P,
    ) -> core::result::Result<T, FrameError<E>>
    where
        Header: Framed,
        H: for<'a> Parser<Input<'a>, Header, E>,
        P: FnOnce(Header) -> F,
        F: for<'a> Parser<Input<'a>, T, E>,
    {
        data.resize(Header::LEN, 0);
        input.read_exact(data).map_err(FrameError::Io)?;
        let mut prefix = data.as_slice();
        let header = header.parse_next(&mut prefix).map_err(FrameError::Error)?;
        data.resize(header.total(), 0);
        input.read_exact(&mut data[Header::LEN..]).map_err(FrameError::Io)?;
        let mut body = &data[Header::LEN..];
        parser(header).parse_next(&mut body).map_err(FrameError::Error)
    }
}

#[cfg(test)]
pub mod twic {
    pub fn data(issue: u16, suffix: &str) -> Vec<u8> {
        let path = path(issue, suffix);
        let error = |error| panic!("cannot read TWIC fixture {}: {error}", path.display());
        std::fs::read(&path).unwrap_or_else(error)
    }

    pub fn path(issue: u16, suffix: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("twic/twic{issue}.{suffix}"))
    }
}
