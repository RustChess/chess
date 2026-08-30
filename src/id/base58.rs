use std::{fmt, str};

use super::Id;
use crate::finite::Empty;

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("invalid base58")]
pub struct Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(DeserializeFromStr, SerializeDisplay))]
pub struct Base58 {
    bytes: Vec<u8>,
    string: String,
}

impl Empty for Id {
    const EMPTY: Self = Id(0);
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Base58::encode(&self.0.to_be_bytes()).fmt(f)
    }
}

impl str::FromStr for Id {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = Base58::decode(s)?;
        let bytes: [u8; 16] = bytes.try_into().map_err(|_| Error)?;
        Ok(Self(u128::from_be_bytes(bytes)))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Id {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&self.to_string())
        } else {
            serializer.serialize_bytes(&self.0.to_be_bytes())
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Id {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Visitor;

        struct IdVisitor;

        impl<'de> Visitor<'de> for IdVisitor {
            type Value = Id;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a base58 string or 16 bytes")
            }

            fn visit_str<E>(self, value: &str) -> Result<Id, E>
            where
                E: serde::de::Error,
            {
                value.parse().map_err(E::custom)
            }

            fn visit_bytes<E>(self, value: &[u8]) -> Result<Id, E>
            where
                E: serde::de::Error,
            {
                let bytes: [u8; 16] = value.try_into().map_err(|_| E::custom("invalid id"))?;
                Ok(Id(u128::from_be_bytes(bytes)))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Id, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut bytes = [0u8; 16];
                for (index, byte) in bytes.iter_mut().enumerate() {
                    *byte = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(index, &self))?;
                }
                if seq.next_element::<u8>()?.is_some() {
                    return Err(serde::de::Error::invalid_length(17, &self));
                }
                Ok(Id(u128::from_be_bytes(bytes)))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(IdVisitor)
        } else {
            deserializer.deserialize_bytes(IdVisitor)
        }
    }
}

impl Base58 {
    pub const LETTERS: [u8; 58] = *b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    pub const OCTETS: [u8; 128] = {
        let mut octets = [0xff; 128];
        let mut index = 0;
        while index < Self::LETTERS.len() {
            octets[Self::LETTERS[index] as usize] = index as u8;
            index += 1;
        }
        octets
    };

    pub fn new(bytes: &[u8]) -> Self {
        Self { bytes: bytes.to_vec(), string: Self::encode(bytes) }
    }

    pub fn parse(s: &str) -> Result<Self> {
        s.parse()
    }

    pub fn random_bytes(len: usize) -> Self {
        use rand::RngExt as _;

        let mut vec = vec![0u8; len];
        rand::rng().fill(vec.as_mut_slice());
        Self::new(&vec)
    }

    pub fn random_str(len: usize) -> Self {
        use rand::RngExt as _;

        let mut rng = rand::rng();
        let string = (0..len)
            .map(|_| Self::LETTERS[rng.random_range(0..Self::LETTERS.len())])
            .collect::<Vec<_>>();
        str::from_utf8(&string).unwrap().parse().unwrap()
    }

    pub fn encode(bytes: &[u8]) -> String {
        let zeros = bytes.iter().take_while(|&&byte| byte == 0).count();
        let mut digits = Vec::<u8>::new();
        for &byte in bytes {
            let mut carry = byte as u32;
            for digit in &mut digits {
                carry += (*digit as u32) << 8;
                *digit = (carry % 58) as u8;
                carry /= 58;
            }
            while carry > 0 {
                digits.push((carry % 58) as u8);
                carry /= 58;
            }
        }

        let mut string = String::with_capacity(zeros + digits.len());
        string.extend(core::iter::repeat_n('1', zeros));
        string.extend(digits.iter().rev().map(|digit| Self::LETTERS[*digit as usize] as char));
        string
    }

    pub fn decode(s: &str) -> Result<Vec<u8>> {
        let zeros = s.bytes().take_while(|&byte| byte == b'1').count();
        let mut bytes = Vec::<u8>::new();
        for byte in s.bytes() {
            if byte as usize >= Self::OCTETS.len() {
                return Err(Error);
            }
            let digit = Self::OCTETS[byte as usize];
            if digit == 0xff {
                return Err(Error);
            }
            let mut carry = digit as u32;
            for decoded in &mut bytes {
                carry += (*decoded as u32) * 58;
                *decoded = (carry & 0xff) as u8;
                carry >>= 8;
            }
            while carry > 0 {
                bytes.push((carry & 0xff) as u8);
                carry >>= 8;
            }
        }

        let mut decoded = vec![0; zeros];
        decoded.extend(bytes.iter().rev());
        Ok(decoded)
    }

    pub const fn as_bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }

    pub const fn as_str(&self) -> &str {
        self.string.as_str()
    }
}

impl str::FromStr for Base58 {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self { bytes: Self::decode(s)?, string: s.to_string() })
    }
}

impl fmt::Display for Base58 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.string.fmt(f)
    }
}

impl From<Base58> for String {
    fn from(base58: Base58) -> Self {
        base58.string
    }
}

#[cfg(test)]
mod tests {
    use super::{Base58, Id};

    #[test]
    fn id_display_roundtrips() {
        for id in [Id(0), Id(1), Id(u128::MAX), Id(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210)] {
            assert_eq!(id.to_string().parse::<Id>().unwrap(), id);
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn json_serializes_id_as_base58_string() {
        let id = Id(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210);
        let json = serde_json::to_string(&id).unwrap();
        assert!(json.starts_with('"'));
        assert_eq!(serde_json::from_str::<Id>(&json).unwrap(), id);
    }

    #[test]
    fn known_vectors() {
        let vectors: &[(&[u8], &str)] = &[
            (&[], ""),
            (&[0], "1"),
            (&[0, 0, 0, 1], "1112"),
            (&[0x61], "2g"),
            (&[0x62, 0x62, 0x62], "a3gV"),
            (&[0x63, 0x63, 0x63], "aPEr"),
            (
                &[
                    0x73, 0x69, 0x6d, 0x70, 0x6c, 0x79, 0x20, 0x61, 0x20, 0x6c, 0x6f, 0x6e, 0x67,
                    0x20, 0x73, 0x74, 0x72, 0x69, 0x6e, 0x67,
                ],
                "2cFupjhnEsSn59qHXstmK2ffpLv2",
            ),
        ];

        for (bytes, encoded) in vectors {
            assert_eq!(Base58::encode(bytes), *encoded);
            assert_eq!(Base58::decode(encoded).unwrap(), *bytes);
            let base58 = Base58::new(bytes);
            assert_eq!(base58.as_str(), *encoded);
            assert_eq!(base58.as_bytes(), *bytes);
        }
    }

    #[test]
    fn rejects_invalid_characters() {
        for invalid in ["0", "O", "I", "l", "+", "/", "abc0"] {
            assert!(Base58::decode(invalid).is_err());
            assert!(Base58::parse(invalid).is_err());
        }
    }
}
