pub use std::collections::BTreeSet as Set;

pub use winnow::ModalResult;
pub use winnow::Parser;

/// Slice of bytes.
pub type ByteInput<'a> = &'a [u8];

/// Slice of UTF-8 text.
pub type StrInput<'a> = &'a str;

pub(crate) mod prelude {
    pub use winnow::{
        ModalResult,
        Parser as _,
        ascii::{dec_uint, multispace0, multispace1, space0, space1},
        binary::{be_u16, be_u24, be_u32, i8, le_i32, le_u16, le_u24, le_u32, u8},
        combinator::{
            alt, cut_err, delimited, opt, preceded, repeat, separated, separated_pair, seq,
            terminated,
        },
        error::{ErrMode, StrContext},
        // stream::Bytes,
        token::{any, none_of, one_of, take, take_till, take_while},
    };
}

mod bits;
pub use bits::Bits;

pub mod cbg;
pub mod cbh;
pub mod cbp;
pub mod cbt;
pub mod cbv;
pub mod fen;
pub mod pgn;
pub mod san;
pub mod uci;
