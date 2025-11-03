// TODO: Use `winnow` machinery?
// Note their note on not keeping "spare bits"
// when converting back to byte parsers
/// Bits from bytes
#[derive(Debug)]
pub struct Bits<'a> {
    /// TODO: make this &mut and actually consume the bits/bytes
    bytes: &'a [u8],
    bit: u8,
}

impl<'a> From<&'a [u8]> for Bits<'a> {
    fn from(bytes: &'a [u8]) -> Self {
        Self { bytes, bit: 0 }
    }
}

impl Iterator for Bits<'_> {
    type Item = bool;

    fn next(&mut self) -> Option<bool> {
        if self.bytes.is_empty() {
            return None;
        }

        if self.bit == 8 {
            self.bytes = &self.bytes[1..];
            if self.bytes.is_empty() {
                return None;
            }
            self.bit = 0;
        }

        let set = self.bytes[0] & (0x80 >> self.bit) != 0;
        self.bit += 1;
        Some(set)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = Bits::len(self);
        (len, Some(len))
    }
}

impl ExactSizeIterator for Bits<'_> {}

impl<'a> Bits<'a> {
    /// Turn bytes into bits
    pub fn new(bytes: &'a [u8]) -> Self {
        bytes.into()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remaining number of bits
    pub fn len(&self) -> usize {
        (8 - self.bit as usize) + 8 * self.bytes.len()
    }

    /// Skip over `n` bits
    pub fn skip(mut self, n: usize) -> Self {
        for _ in 0..n {
            self.next().unwrap();
        }
        self
    }

    /// Take `n` bits, interpreted as big-endian unsigned integer
    pub fn unsigned(&mut self, n: usize) -> Option<usize> {
        let mut unsigned = 0;

        for _ in 0..n {
            unsigned *= 2;
            unsigned += self.next()? as usize;
        }

        Some(unsigned)
    }
}

#[test]
fn bits() {
    let bits: &[u8] = &[];
    let bits = Bits::from(bits);
    assert!(bits.collect::<Vec<bool>>().is_empty());
    let bits = Bits::from([0b1000_1111u8].as_slice());
    assert_eq!(
        vec![true, false, false, false, true, true, true, true],
        bits.collect::<Vec<bool>>()
    );
}
