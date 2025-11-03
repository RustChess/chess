pub use std::collections::BTreeSet as Set;

pub use winnow::Parser;

/// Slice of bytes
pub type Input<'a> = &'a [u8];

pub(crate) mod prelude {
    pub use super::Input;

    pub use winnow::{
        ModalResult,
        Parser as _,
        ascii::{dec_uint, space0, space1},
        binary::{be_u16, be_u24, be_u32, /*bits,*/ i8, le_i32, le_u16, le_u24, le_u32, u8},
        combinator::{alt, cut_err, opt, preceded, repeat, /*seq,*/ terminated},
        // stream::Bytes,
        token::{one_of, take},
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
