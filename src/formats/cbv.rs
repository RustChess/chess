//! Chessbase archive file format

// https://github.com/antoyo/uncbv
// https://github.com/antoyo/uncbv/issues/3
// https://github.com/antoyo/uncbv/issues/3

// Note: It's called cbV for archiVe (or also version

// TODO: We should have a constructor that looks like a map
// and offers Index-based access to the packed files (as references).
// The packed file then has a `PackedFile::unpack` method.
//
// Implement this with `fs::File::seek` operations and streaming
// processing, using the `indexmap` crate.

use core::ffi::c_str::CStr;
use std::path::Path;

use encoding_rs::WINDOWS_1252;

use super::{Bits, ByteInput as Input, prelude::*};

mod huffman;
pub use huffman::Tree;

/////////////
// Parsing //
/////////////

/// Vector of entries
pub type Header = Vec<Entry>;

// 8 byte header
// - 2B magic number tag
// - 2B number of files
// - 1B length of metadata entry
// - 3B unknown
//
// Metadata for each file (all same length e.g. 173 bytes)
// - offset   0: C-string filename (Windows1252 encoding?)
// - offset 132..136: 4B packed size
// - offset 136..140: 4B unpacked size
/// Parse the header out of a Chessbase archive
pub fn header(input: &mut Input<'_>) -> ModalResult<Vec<Entry>> {
    let (_, file_count, entry_len, _) = (magic_tag, le_u16, u8, take(3u8)).parse_next(input)?;
    let header: Vec<Entry> =
        repeat(..=file_count as usize, cut_err(take(entry_len).and_then(entry)))
            .parse_next(input)?;
    Ok(header)
}

/// Name and packed/unpacked lengths of an archive entry
#[derive(Debug)]
pub struct Entry {
    /// Name of the file
    ///
    /// This can contain embedded Windows-style backslash paths,
    /// e.g., `D85 Gruenfeld Defence-163.html\42289499p0.jpg`
    pub name: String,
    /// The length of the packed data, in bytes
    pub packed: usize,
    /// The length of the (original) data, in bytes
    pub len: usize,
}

fn entry(input: &mut Input<'_>) -> ModalResult<Entry> {
    // TODO: pass along errors
    let name = take(132usize).map(cstr).parse_next(input)?;

    // offsets 132 and 136
    let (packed, len) = (le_i32.map(as_usize), le_i32.map(as_usize)).parse_next(input)?;

    let _ = take(1usize).parse_next(input)?;

    // offsets 141 and 145
    // let last_modified_date = (le_u32, le_u32).parse_next(input)?;
    // let (mdate, mtime) = (le_u32, le_u32).parse_next(input)?;
    // println!("{} and {:08}", mdate, mtime);

    Ok(Entry { name, packed, len })
}

/// Packed content of a file
pub struct PackedFile<'a> {
    pub name: String,
    pub data: &'a [u8],
    pub len: usize,
}

// This is the way to pass arguments:
// return a Parser that is implemented in terms of the arguments
pub fn packed_file<'a>(entry: &Entry) -> impl FnMut(&mut Input<'a>) -> ModalResult<PackedFile<'a>> {
    move |input: &mut Input<'_>| {
        take(entry.packed)
            .map(|data: &[u8]| PackedFile { name: entry.name.clone(), data, len: entry.len })
            .parse_next(input)
    }
}

///////////////
// Unpacking //
///////////////

pub fn unpack_cbv_to_disk(input: &mut Input<'_>) -> ModalResult<Header> {
    unpack_cbv_to(Path::new(".")).parse_next(input)
}

pub fn unpack_cbv_to<'a>(directory: &'a Path) -> impl FnMut(&mut Input<'a>) -> ModalResult<Header> {
    move |input: &mut Input<'_>| {
        let header = header.parse_next(input)?;
        for file in header.iter() {
            let mut packed: PackedFile = packed_file(file).parse_next(input)?;
            let unpacked = unpack_file.parse_next(&mut packed.data)?;
            assert_eq!(file.len, unpacked.len());
            let path = directory.join(&file.name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("can create output directory");
            }
            std::fs::write(path, &unpacked).expect("can write");
        }
        Ok(header)
    }
}

// #[cfg(test)]
// fn unpack_cbv_dummy(input: &mut Input<'_>) -> ModalResult<Header> {
//     let header = header.parse_next(input)?;
//     for file in header.iter() {
//         let mut packed: PackedFile = packed_file(file).parse_next(input)?;
//         let unpacked = unpack_file.parse_next(&mut packed.data)?;
//         assert_eq!(file.len, unpacked.len());
//     }
//     Ok(header)
// }

pub fn unpack_file(input: &mut Input<'_>) -> ModalResult<Vec<u8>> {
    let mut data = Vec::new();
    while !input.is_empty() {
        // 1. Read out a block
        let (packing, mut block) = read_block.parse_next(input)?;

        // 2. Remove Huffman encoding
        let block = if packing.huffman {
            unpack_huffman(&mut block).expect("correct Huffman")
        } else {
            block.to_vec()
        };

        // 3. Remove RLE/backref encoding
        let mut block = if packing.rle {
            unpack_rle_backref(&mut block.as_slice()).expect("correct RLE/backref")
        } else {
            block
        };
        data.append(&mut block);
    }
    Ok(data)
}

/// Encodings applied to a block of a packed file
#[derive(Clone, Copy, Debug)]
pub struct Packing {
    /// Huffman-tree encoding
    pub huffman: bool,
    /// Mix of run-time length encoding and backward references
    pub rle: bool,
}

pub fn read_block<'a>(input: &mut Input<'a>) -> ModalResult<(Packing, &'a [u8])> {
    // let (file_count, entry_len) =
    //     seq!(_: magic_tag, le_u16, u8, _: take(3usize)).parse_next(input)?;
    let (block_size, _) = (le_u16, take(2usize)).parse_next(input)?;
    let (packing, block) = take(block_size as usize).parse_next(input)?.split_first().unwrap();
    assert!(*packing < 4);
    let rle = packing & 1 != 0;
    let huffman = packing & 2 != 0;
    Ok((Packing { huffman, rle }, block))
}

// https://reverseengineering.stackexchange.com/questions/8593/unknown-decompression-algorithm/8601
pub fn unpack_huffman(input: &mut Input<'_>) -> winnow::Result<Vec<u8>> {
    use core::array::from_fn;

    // 1. unpacked length
    let len = be_u16(input)? as usize;

    // 2. Huffman tree
    let mut bits = Bits::from(*input);
    let values: [(usize, u16); 256] = from_fn(|_| huffman_node(&mut bits).unwrap());

    // 3. Decode block with tree
    let tree = Tree::load(values);
    let unpacked = tree.unpack(bits, len).unwrap();

    assert_eq!(len, unpacked.len());
    Ok(unpacked)
}

fn huffman_node(bits: &mut Bits) -> Option<(usize, u16)> {
    let len = bits.unsigned(4)?;
    let bits = bits.unsigned(len)? as u16;
    Some((len, bits))
}

// This has run-length encoding and backward references
pub fn unpack_rle_backref(input: &mut Input<'_>) -> winnow::Result<Vec<u8>> {
    use winnow::binary::{le_u16, u8};

    let mut result = vec![];
    while !input.is_empty() {
        let indicator = le_u16(input)?.to_be_bytes();
        let indicator = Bits::from(indicator.as_slice());

        for encoded in indicator {
            if input.is_empty() {
                return Ok(result);
            }

            if !encoded {
                result.push(u8(input)?);
                continue;
            }

            let (high, low) = u8(input).map(nibbles)?;

            match high {
                // run-length encoding, 3..=18
                0 => {
                    result.append(&mut vec![u8(input)?; 3 + low]);
                }
                // run-length encoding, 19..=4114
                1 => {
                    let size = 19 + low + ((u8(input)? as usize) << 4);
                    result.append(&mut vec![u8(input)?; size]);
                }
                // backward reference
                _ => {
                    // 3..=4098
                    let start = result.len() - (3 + low + ((u8(input)? as usize) << 4));
                    let end = start
                        + if high > 2 {
                            // 3..=15
                            high as usize
                        } else {
                            // 16..=271
                            16 + u8(input)? as usize
                        };
                    result.extend_from_within(start..end);
                }
            }
        }
    }

    Ok(result)
}

fn nibbles(x: u8) -> (u8, usize) {
    (x >> 4, x as usize & 0xF)
}

////////////////////////////
// Implementation details //
////////////////////////////

fn magic_tag(input: &mut Input<'_>) -> ModalResult<[u8; 2]> {
    const MAGIC_NUMBER: [u8; 2] = [0x08, 0x00];
    MAGIC_NUMBER.as_slice().map(|_| MAGIC_NUMBER).parse_next(input)
}

fn as_usize(x: i32) -> usize {
    x as usize
}

pub fn cstr(input: &[u8]) -> String {
    let name = CStr::from_bytes_until_nul(input).expect("valid name");
    let (cow, encoding_used, had_errors) = WINDOWS_1252.decode(name.to_bytes());
    assert_eq!(encoding_used, WINDOWS_1252);
    assert!(!had_errors);
    cow.to_string()
}

///////////
// Tests //
///////////

#[test]
fn example() {
    const EXAMPLE: &[u8] = include_bytes!("../../examples/twic1616.cbv");

    // let header = unpack_cbv_dummy.parse(EXAMPLE).unwrap();
    let header = unpack_cbv_to_disk.parse(EXAMPLE).unwrap();
    println!("{header:?}");
}
