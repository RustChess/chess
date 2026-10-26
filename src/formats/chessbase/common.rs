use crate::formats::bytes_prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub first_record: usize,
    pub len: usize,
    pub avl_root: u32,
    pub first_deleted: Option<usize>,
    pub record_len: usize,
    pub len_existing: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub deleted: bool,
}

pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    let len = le_u32.parse_next(input)? as usize;
    let avl_root = le_u32.parse_next(input)?;
    let _one_to_zero = le_u32.verify(|value| *value == 1234567890).parse_next(input)?;
    let record_len = le_u32.parse_next(input)? as usize;
    let first_deleted = le_i32.map(|x| (x != -1).then_some(x as usize)).parse_next(input)?;
    let len_existing = le_u32.parse_next(input)? as usize;
    let more_offset = le_u32.verify(|offset| matches!(*offset, 0 | 4)).parse_next(input)?;
    let _padding = take(more_offset)
        .verify(|padding: &[u8]| padding.iter().all(|byte| *byte == 0))
        .parse_next(input)?;

    Ok(Header {
        first_record: 28 + more_offset as usize,
        len,
        avl_root,
        first_deleted,
        record_len,
        len_existing,
    })
}

pub fn node(input: &mut Input<'_>) -> ModalResult<Node> {
    let (left, deleted) = le_i32
        .verify_map(|index| {
            if index == -999 {
                Some((None, true))
            } else {
                optional_index(index).map(|index| (index, false))
            }
        })
        .parse_next(input)?;
    let (right, next_deleted) = le_i32
        .verify_map(|index| {
            if deleted {
                optional_index(index).map(|index| (None, index))
            } else {
                optional_index(index).map(|index| (index, None))
            }
        })
        .parse_next(input)?;
    let _balance = i8.verify(|balance| !deleted || *balance == 0).parse_next(input)?;
    let _ = (left, right, next_deleted);

    Ok(Node { deleted })
}

fn optional_index(index: i32) -> Option<Option<usize>> {
    match index {
        -1 => Some(None),
        0.. => Some(Some(index as usize)),
        _ => None,
    }
}
