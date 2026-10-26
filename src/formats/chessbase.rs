//! ChessBase database formats.

// Format references:
// - https://talkchess.com/viewtopic.php?t=29468
// - https://github.com/foolnotion/scidb/tree/master/src/db/cbh

use encoding_rs::WINDOWS_1252;

use super::Text;

pub mod annotation;
pub mod annotator;
pub mod archive;
pub mod common;
pub mod country;
pub mod database;
pub mod game;
pub mod huffman;
pub mod index;
pub mod player;
pub mod secondary;
pub mod source;
pub mod team;
pub mod tournament;

// Decode a fixed-width C character array, whether or not it contains a NUL terminator.
fn cstr(input: &[u8]) -> String {
    let end = input.iter().position(|byte| *byte == 0).unwrap_or(input.len());
    string(&input[..end])
}

fn string(input: &[u8]) -> String {
    let (value, encoding, had_errors) = WINDOWS_1252.decode(input);
    assert_eq!(encoding, WINDOWS_1252);
    assert!(!had_errors);
    value.to_string()
}

fn text(input: &[u8]) -> Option<Text> {
    Text::new(cstr(input)).ok()
}

// Numbers where zero means missing or unknown.
fn optional<T: Default + PartialEq>(value: T) -> Option<T> {
    (value != T::default()).then_some(value)
}
