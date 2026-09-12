//! Chessbase archive file format

// https://github.com/antoyo/uncbv
// https://github.com/antoyo/uncbv/issues/3
// https://github.com/antoyo/uncbv/issues/3

// Note: It's called cbV for archiVe (or also version

use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::formats::{Bits, ByteInput as Input, prelude::*};

use super::{cstr, huffman::Tree};

/// ChessBase archive filename suffix.
pub const SUFFIX: &str = "cbv";

pub type Range = core::range::Range<usize>;

crate::finite_set!(
    /// A recognized ChessBase archive member.
    Kind,
    /// Archive members by kind.
    Members {
        Games as games,
        Index as index,
        Players as players,
        Tournaments as tournaments,
        Annotations as annotations,
        Annotators as annotators,
        Sources as sources,
        Teams as teams,
    }
);

/////////////
// Parsing //
/////////////

/// Archive directory framing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Info {
    pub members: usize,
    pub metadata_len: usize,
}

/// ChessBase archive header.
#[derive(Debug)]
pub struct Header {
    pub len: usize,
    pub members: Vec<Member>,
}

impl Info {
    pub const LEN: usize = 8;

    pub fn header_len(self) -> usize {
        self.members * self.metadata_len + Self::LEN
    }
}

/// A file contained in a ChessBase archive.
#[derive(Clone, Debug)]
pub struct Member {
    /// Member filename.
    ///
    /// This can contain embedded Windows-style backslash paths,
    /// e.g. `D85 Gruenfeld Defence-163.html\42289499p0.jpg`.
    pub name: String,
    /// Unpacked member length.
    pub len: usize,
    /// Packed location within the archive.
    pub packed: Range,
}

/// Packed and unpacked locations of an archive compression block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Block {
    pub packed: Range,
    pub unpacked: Range,
}

/// Random access to members packed in a ChessBase archive.
pub struct Access<I> {
    input: I,
    members: Members<Option<State>>,
    packed: Vec<u8>,
}

/// An unpacked view of one member in a ChessBase archive.
pub struct Reader<'a, I> {
    input: &'a mut I,
    state: &'a mut State,
    packed: &'a mut Vec<u8>,
    position: usize,
}

struct State {
    member: Member,
    blocks: Vec<Block>,
    cached: Option<(usize, Vec<u8>)>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid ChessBase archive")]
    Data,
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<FrameError<ContextMode>> for Error {
    fn from(error: FrameError<ContextMode>) -> Self {
        match error {
            FrameError::Error(_) => Self::Data,
            FrameError::Io(error) => Self::Io(error),
        }
    }
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl Framed for Info {
    const LEN: usize = Info::LEN;

    fn total(&self) -> usize {
        self.header_len()
    }
}

pub fn read_header(input: &mut impl Read) -> Result<Header> {
    Ok(framed(input, &mut Vec::new(), info, members)?)
}

// 8 byte header
// - 2B magic number tag
// - 2B number of members
// - 1B length of each metadata record
// - 3B unknown
//
// Metadata for each member (all same length e.g. 173 bytes)
// - offset   0: C-string filename (Windows1252 encoding?)
// - offset 132..136: 4B packed size
// - offset 136..140: 4B unpacked size
// - offset 140: unknown
// - offsets 141 and 145: possibly last-modified date and time
/// Parse the header out of a Chessbase archive
pub fn header(input: &mut Input<'_>) -> ModalResult<Header> {
    let info = info.parse_next(input)?;
    members(info).parse_next(input)
}

fn members(info: Info) -> impl FnMut(&mut Input<'_>) -> ModalResult<Header> {
    move |input| {
        let len = info.header_len();
        let mut offset = len;
        let mut members = Vec::with_capacity(info.members);
        // The archive stores relative member layout; retain absolute offsets for access.
        for _ in 0..info.members {
            let member = member(offset, info.metadata_len).parse_next(input)?;
            offset = member.packed.end;
            members.push(member);
        }
        Ok(Header { len, members })
    }
}

pub fn info(input: &mut Input<'_>) -> ModalResult<Info> {
    let (members, metadata_len) =
        delimited(magic_tag, (le_u16, u8), take(3u8)).parse_next(input)?;
    Ok(Info { members: members as usize, metadata_len: metadata_len as usize })
}

fn member(offset: usize, metadata_len: usize) -> impl FnMut(&mut Input<'_>) -> ModalResult<Member> {
    move |input: &mut Input<'_>| {
        let metadata = take(metadata_len).and_then(metadata).parse_next(input)?;
        let end = offset.checked_add(metadata.packed).ok_or_else(backtrack)?;
        Ok(Member { name: metadata.name, len: metadata.len, packed: Range::from(offset..end) })
    }
}

struct Metadata {
    name: String,
    len: usize,
    packed: usize,
}

fn metadata(input: &mut Input<'_>) -> ModalResult<Metadata> {
    terminated(
        seq! {Metadata {
            name: take(132usize).map(cstr),
            packed: le_i32.map(as_usize),
            len: le_i32.map(as_usize),
        }},
        take(1usize),
    )
    .parse_next(input)
}

///////////////
// Uncompression //
///////////////

pub fn unpack_to(member: &Member, input: &mut impl Read, output: &mut impl Write) -> Result<()> {
    let mut input = input.take((member.packed.end - member.packed.start) as u64);
    let mut packed = Vec::new();
    let mut len = 0usize;
    while input.limit() != 0 {
        let block = framed(&mut input, &mut packed, block_info, unpack_block)?;
        len = len.checked_add(block.len()).ok_or(Error::Data)?;
        output.write_all(&block)?;
    }
    if len != member.len {
        return Err(Error::Data);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct BlockInfo {
    len: usize,
}

impl Framed for BlockInfo {
    const LEN: usize = 4;

    fn total(&self) -> usize {
        Self::LEN + self.len
    }
}

/// Encodings applied to a block of a packed member.
#[derive(Clone, Copy, Debug)]
pub struct Compression {
    /// Huffman-tree encoding
    pub huffman: bool,
    /// Mix of run-time length encoding and backward references
    pub rle: bool,
}

fn block_info(input: &mut Input<'_>) -> ModalResult<BlockInfo> {
    terminated(le_u16, take(2u8))
        .verify(|len| *len > 0)
        .map(|len| BlockInfo { len: usize::from(len) })
        .parse_next(input)
}

fn unpack_block(info: BlockInfo) -> impl FnMut(&mut Input<'_>) -> ModalResult<Vec<u8>> {
    move |input| {
        let compression = compression.parse_next(input)?;
        let mut block = take(info.len - 1).parse_next(input)?;

        let block = if compression.huffman {
            unpack_huffman(&mut block).expect("correct Huffman")
        } else {
            block.to_vec()
        };

        if compression.rle {
            unpack_rle_backref(&mut block.as_slice()).map_err(ErrMode::Backtrack)
        } else {
            Ok(block)
        }
    }
}

fn compression(input: &mut Input<'_>) -> ModalResult<Compression> {
    u8.verify_map(|compression| {
        (compression < 4)
            .then_some(Compression { rle: compression & 1 != 0, huffman: compression & 2 != 0 })
    })
    .parse_next(input)
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

impl<I> Access<I> {
    pub fn new(input: I, members: &[Member]) -> Result<Self> {
        let mut states: Members<Option<State>> = Members::empty();
        for member in members {
            let Some(kind) = Kind::from_name(&member.name) else {
                continue;
            };
            if states[kind].is_some() {
                return Err(Error::Data);
            }
            states[kind] = Some(State { member: member.clone(), blocks: Vec::new(), cached: None });
        }
        Ok(Self { input, members: states, packed: Vec::new() })
    }

    pub fn reader(&mut self, kind: Kind) -> Option<Reader<'_, I>> {
        let state = self.members[kind].as_mut()?;
        Some(Reader { input: &mut self.input, state, packed: &mut self.packed, position: 0 })
    }
}

impl Kind {
    fn from_name(name: &str) -> Option<Self> {
        Self::from_suffix(Path::new(name).extension()?.to_str()?)
    }

    fn from_suffix(suffix: &str) -> Option<Self> {
        use Kind::*;

        Some(match suffix.to_ascii_lowercase().as_str() {
            "cbg" => Games,
            "cbh" => Index,
            "cbp" => Players,
            "cbt" => Tournaments,
            "cba" => Annotations,
            "cbc" => Annotators,
            "cbs" => Sources,
            "cbe" => Teams,
            _ => return None,
        })
    }
}

impl<I: Read + Seek> Reader<'_, I> {
    fn locate(&mut self, position: usize) -> Result<(usize, Block)> {
        if let Some((i, block)) = self
            .state
            .blocks
            .iter()
            .enumerate()
            .find(|(_, block)| block.unpacked.contains(&position))
        {
            return Ok((i, *block));
        }

        loop {
            let (packed_offset, unpacked_offset, index) =
                if let Some(block) = self.state.blocks.last() {
                    (block.packed.end, block.unpacked.end, self.state.blocks.len())
                } else {
                    (0, 0, 0)
                };

            if packed_offset >= self.state.member.packed.end - self.state.member.packed.start {
                return Err(Error::Data);
            }

            let (block, unpacked) = self.discover(packed_offset, unpacked_offset)?;
            self.state.blocks.push(block);
            self.state.cached = Some((index, unpacked));
            if block.unpacked.contains(&position) {
                return Ok((index, block));
            }
        }
    }

    fn discover(
        &mut self,
        packed_offset: usize,
        unpacked_offset: usize,
    ) -> Result<(Block, Vec<u8>)> {
        let absolute =
            self.state.member.packed.start.checked_add(packed_offset).ok_or(Error::Data)?;
        self.input.seek(SeekFrom::Start(absolute as u64))?;
        let remaining = (self.state.member.packed.end - self.state.member.packed.start)
            .checked_sub(packed_offset)
            .ok_or(Error::Data)?;
        let mut input = self.input.by_ref().take(remaining as u64);
        let data = framed(&mut input, self.packed, block_info, unpack_block)?;
        let packed = Range {
            start: packed_offset,
            end: packed_offset.checked_add(self.packed.len()).ok_or(Error::Data)?,
        };
        let unpacked = Range {
            start: unpacked_offset,
            end: unpacked_offset.checked_add(data.len()).ok_or(Error::Data)?,
        };

        Ok((Block { packed, unpacked }, data))
    }

    fn load(&mut self, index: usize, block: Block) -> Result<()> {
        if self.state.cached.as_ref().map(|(cached, _)| *cached) == Some(index) {
            return Ok(());
        }

        let (loaded, unpacked) = self.discover(block.packed.start, block.unpacked.start)?;
        if loaded != block {
            return Err(Error::Data);
        }
        self.state.cached = Some((index, unpacked));
        Ok(())
    }
}

impl<I: Read + Seek> Read for Reader<'_, I> {
    fn read(&mut self, mut output: &mut [u8]) -> io::Result<usize> {
        let start_len = output.len();
        while !output.is_empty() && self.position < self.state.member.len {
            let (index, block) = self.locate(self.position)?;
            self.load(index, block)?;
            let offset = self.position - block.unpacked.start;
            let available = block.unpacked.end - self.position;
            let len = available.min(output.len());
            let unpacked = &self.state.cached.as_ref().ok_or_else(invalid_data)?.1;
            output[..len].copy_from_slice(&unpacked[offset..offset + len]);
            self.position += len;
            output = &mut output[len..];
        }
        Ok(start_len - output.len())
    }
}

impl<I> Seek for Reader<'_, I> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        use SeekFrom::*;

        let base = match position {
            Start(position) => {
                self.position = usize::try_from(position).map_err(|_| invalid_input())?;
                return Ok(position);
            }
            Current(_) => self.position,
            End(_) => self.state.member.len,
        };
        let offset = match position {
            Current(offset) | End(offset) => offset,
            Start(_) => unreachable!(),
        };
        let position = (base as i128).checked_add(i128::from(offset)).ok_or_else(invalid_input)?;
        self.position = usize::try_from(position).map_err(|_| invalid_input())?;
        Ok(self.position as u64)
    }
}

impl From<Error> for io::Error {
    fn from(error: Error) -> io::Error {
        match error {
            Error::Data => io::Error::new(io::ErrorKind::InvalidData, error),
            Error::Io(error) => error,
        }
    }
}

fn invalid_input() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "invalid ChessBase archive seek")
}

fn invalid_data() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, Error::Data)
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::formats::twic;

    use super::*;

    #[test]
    fn unpack_to_rle() {
        const INPUT: [u8; 38] = hex!(
            "22004d39013740010400d20296494806000001ff410104040005ff1e62000008
             6b1800004665"
        );
        const PREFIX: [u8; 40] = hex!(
            "0100000000000000d202964948060000ffffffff010000000400000000000000
             ffffffffffffffff"
        );
        const SUFFIX: [u8; 8] = hex!("6b18000001000000");

        let member = Member {
            name: "twic1616.cbl".to_string(),
            len: 1649,
            packed: Range::from(0..INPUT.len()),
        };
        let mut input = INPUT.as_slice();
        let mut output = Vec::new();
        super::unpack_to(&member, &mut input, &mut output).unwrap();

        let mut expected = vec![0; member.len];
        expected[..PREFIX.len()].copy_from_slice(&PREFIX);
        expected[member.len - SUFFIX.len()..].copy_from_slice(&SUFFIX);
        assert_eq!(output, expected);
    }

    #[test]
    fn unpack_to_huffman_rle() {
        const INPUT: [u8; 295] = hex!(
            "23014bce03013d2190085a756ebe120f0000000428429001b0084684984a3a10
             8a10a210aa007000000002138000000002130000000002134211c00000000000
             21600000000000000000000002170000000021740000000000000000216c0000
             0000000000000002164000000000000000000000000000000000212c00000000
             000000000000000000000211000000213c0000214c0000000000215000000000
             000000fab475e0b28027202e2e8a6ad685b554d4558504f4c41243612cc64149
             18444a4825ea2bcfe5ef12f7897bc4bde25ef12f7897bc4bde3f97bc4bde25ef
             12f7897bc4bde25ef12f78fe5ef12f7897bc4bde25ef12f7897bc4bde3f97bc4
             bde25ef12f7897bc4bde25ef12f78fe5ef12f7897bc4bde25ef12f7897bc4bde
             05497bc4beaf00"
        );

        let member = Member {
            name: "twic1616.cbtt".to_string(),
            len: 17_852,
            packed: Range::from(0..INPUT.len()),
        };
        let mut input = INPUT.as_slice();
        let mut output = Vec::new();
        super::unpack_to(&member, &mut input, &mut output).unwrap();

        assert_eq!(output.len(), member.len);
        assert_eq!(crc32(&output), 0xb007_79cb);
    }

    fn crc32(input: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in input {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
            }
        }
        !crc
    }

    #[test]
    #[ignore = "exhaustive test against the external TWIC fixture"]
    fn twic_unpack_to() {
        let data = twic::data(1616, "cbv");
        let mut input = data.as_slice();
        let header = read_header(&mut input).unwrap();
        for member in &header.members {
            let mut output = Vec::new();
            super::unpack_to(member, &mut input, &mut output).unwrap();
        }
        println!("{header:?}");
    }
}
