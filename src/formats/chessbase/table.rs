use crate::formats::{ByteInput as Input, prelude::*};

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub first_record: usize,
    pub len: usize,
    pub avl_root: u32,
    pub first_deleted: Option<usize>,
    pub record_len: usize,
    pub len_existing: usize,
}

pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    let len = le_u32.parse_next(input)? as usize;
    let avl_root = le_u32.parse_next(input)?;
    let _one_to_zero = le_u32.verify(|value| *value == 1234567890).parse_next(input)?;
    let record_len = le_u32.parse_next(input)? as usize;
    // Not true for CBT which reuses the header
    // println!("size: {size}");
    // assert_eq!(67, size + 9);
    let first_deleted = le_i32.map(|x| (x != -1).then_some(x as usize)).parse_next(input)?;
    let len_existing = le_u32.parse_next(input)? as usize;
    // assert!(len_existing >= len);
    // padding? is 4 in an example
    let more_offset = le_u32.verify(|offset| matches!(*offset, 0 | 4)).parse_next(input)?;
    // println!("more offset: {more_offset}");
    // compared to talkchess.com post, there's another four bytes
    let _padding = take(more_offset)
        .verify(|padding: &[u8]| padding.iter().all(|byte| *byte == 0))
        .parse_next(input)?;

    Ok(Header {
        first_record: 28 + more_offset as usize,
        len,
        avl_root,
        record_len,
        first_deleted,
        len_existing,
    })
}
