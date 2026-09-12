//! ChessBase database formats.

// Format references:
// - https://talkchess.com/viewtopic.php?t=29468
// - https://github.com/foolnotion/scidb/tree/master/src/db/cbh

use core::ffi::c_str::CStr;

use encoding_rs::WINDOWS_1252;

pub mod archive;
pub mod database;
pub mod game;
pub mod huffman;
pub mod index;
pub mod player;
pub mod table;
pub mod tournament;

fn cstr(input: &[u8]) -> String {
    let value = CStr::from_bytes_until_nul(input).expect("valid string");
    let (value, encoding, had_errors) = WINDOWS_1252.decode(value.to_bytes());
    assert_eq!(encoding, WINDOWS_1252);
    assert!(!had_errors);
    value.to_string()
}

// Numbers where zero means missing or unknown.
fn optional<T: Default + PartialEq>(value: T) -> Option<T> {
    (value != T::default()).then_some(value)
}
