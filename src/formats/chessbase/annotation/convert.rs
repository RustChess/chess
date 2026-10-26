use core::iter::Peekable;

use std::vec;

use encoding_rs::WINDOWS_1252;

use crate::{
    Player,
    formats::Text,
    game::{CursorMut, Nag},
};

use super::{Annotation, Annotations, Comment, Entry};

#[derive(Debug, thiserror::Error)]
#[error("invalid ChessBase annotation move index {0}")]
pub struct Error(pub usize);

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub struct Apply {
    entries: Peekable<vec::IntoIter<Entry>>,
    index: usize,
}

impl Apply {
    pub fn new(annotations: Option<Annotations>) -> Self {
        let entries = annotations.map_or_else(Vec::new, |annotations| annotations.entries);
        Self { entries: entries.into_iter().peekable(), index: 0 }
    }

    pub fn root(&mut self, cursor: &mut CursorMut<'_>) {
        for annotation in self.take(None) {
            apply_position(cursor, annotation);
        }
    }

    pub fn before(&mut self, cursor: &mut CursorMut<'_>) -> Vec<Annotation> {
        let annotations = self.take(Some(self.index));
        self.index += 1;
        for annotation in &annotations {
            if let Annotation::Before(comment) = annotation {
                add_comment(cursor, comment);
            }
        }
        annotations
    }

    pub fn after(
        &mut self,
        cursor: &mut CursorMut<'_>,
        annotations: Vec<Annotation>,
        player: Player,
    ) {
        for annotation in annotations {
            if !matches!(annotation, Annotation::Before(_)) {
                apply_move(cursor, annotation, player);
            }
        }
    }

    pub fn finish(mut self) -> Result<()> {
        match self.entries.next() {
            Some(entry) => Err(Error(entry.index.unwrap_or(self.index))),
            None => Ok(()),
        }
    }

    fn take(&mut self, index: Option<usize>) -> Vec<Annotation> {
        let mut annotations = Vec::new();
        while self.entries.peek().is_some_and(|entry| entry.index == index) {
            annotations.push(self.entries.next().expect("peeked annotation exists").annotation);
        }
        annotations
    }
}

fn apply_position(cursor: &mut CursorMut<'_>, annotation: Annotation) {
    match annotation {
        Annotation::Before(comment) | Annotation::After(comment) => add_comment(cursor, &comment),
        Annotation::Arrows(arrows) => {
            cursor.position_mut().update_arrows(|current| current.extend(arrows));
        }
        Annotation::Squares(markers) => {
            cursor.position_mut().update_markers(|current| current.extend(markers));
        }
        Annotation::Symbols(_) | Annotation::Unknown { .. } => {}
    }
}

fn apply_move(cursor: &mut CursorMut<'_>, annotation: Annotation, player: Player) {
    match annotation {
        Annotation::After(comment) => add_comment(cursor, &comment),
        Annotation::Arrows(arrows) => {
            cursor.position_mut().update_arrows(|current| current.extend(arrows));
        }
        Annotation::Squares(markers) => {
            cursor.position_mut().update_markers(|current| current.extend(markers));
        }
        Annotation::Symbols(symbols) => cursor
            .play_mut()
            .expect("cursor advanced by a move")
            .update_nags(|nags| nags.extend(decode_nags(&symbols, player))),
        Annotation::Before(_) | Annotation::Unknown { .. } => {}
    }
}

fn add_comment(cursor: &mut CursorMut<'_>, comment: &Comment) {
    let Some(comment) = decode_comment(comment) else { return };
    cursor.position_mut().update_public(|public| match &mut public.comment {
        Some(current) => current.merge(&comment),
        None => public.comment = Some(comment),
    });
}

pub fn decode_comment(comment: &Comment) -> Option<Text> {
    let mut output = String::new();
    let mut plain = Vec::new();
    let mut bytes = comment.text.as_slice();

    while let Some((&byte, rest)) = bytes.split_first() {
        bytes = rest;
        if byte == b'\r' {
            append_plain(&mut output, &mut plain);
            output.push('\n');
            bytes = bytes.strip_prefix(b"\n").unwrap_or(bytes);
        } else if let Some(symbol) = symbol(byte) {
            append_plain(&mut output, &mut plain);
            output.push_str(symbol);
        } else {
            plain.push(byte);
        }
    }
    append_plain(&mut output, &mut plain);
    Text::new(output).ok()
}

fn append_plain(output: &mut String, plain: &mut Vec<u8>) {
    let (text, _) = WINDOWS_1252.decode_without_bom_handling(plain);
    output.push_str(&text);
    plain.clear();
}

fn symbol(byte: u8) -> Option<&'static str> {
    Some(match byte {
        0x82 => "->",
        0x83 => "|^",
        0x84 => "<=>",
        0x85 => "/\\",
        0x86 => "()",
        0x87 => "(.)",
        0x90 => "@",
        0x91 => "<->",
        0x92 => "/^",
        0x93 => "(+)",
        0x94 => "[+]",
        0x99 => "[]",
        0x9e => "#",
        0xa2 => "K",
        0xa3 => "Q",
        0xa4 => "N",
        0xa5 => "B",
        0xa6 => "R",
        0xa7 => "P",
        0xa9 | 0xb0 => "~/=",
        0xaa => "|_",
        0xab => "<<",
        0xac => "_|_",
        0xad => "^^",
        0xae => "^_",
        0xaf => "^=",
        0xb1 => "+/-",
        0xb2 => "+/=",
        0xb3 => "=/+",
        0xb5 => "-/+",
        0xb9 => ">=",
        0xba => "_|",
        0xbb => ">>",
        0xd7 => "><",
        0xf7 => "~~",
        0xfe => "o^",
        _ => return None,
    })
}

pub fn decode_nags(symbols: &[u8], player: Player) -> impl Iterator<Item = Nag> + '_ {
    symbols.iter().enumerate().filter_map(move |(index, symbol)| {
        let nag = match (index, symbol) {
            (0, 1..=6 | 8) => u32::from(*symbol),
            (0, 0x16) => by_player(player, 22),
            (1, 0x0b) => 10,
            (1, 0x0d..=0x13) => u32::from(*symbol),
            (1, 0x20) => by_player(player, 32),
            (1, 0x24) => by_player(player, 36),
            (1, 0x28) => by_player(player, 40),
            (1, 0x2c) => by_player(player, 44),
            (1, 0x84) => by_player(player, 132),
            (1, 0x8a) => 177,
            (1, 0x92) => 146,
            (2, 0x8c..=0x91) => u32::from(*symbol) + 140 - 0x8c,
            _ => return None,
        };
        Some(Nag::Numeric(nag))
    })
}

fn by_player(player: Player, white: u32) -> u32 {
    white + u32::from(player.is_black())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_glyphs() {
        let comment =
            Comment { language: 0, text: b"15...\xa3f2 16.\xa4e4! \xb1\r\nNext".to_vec() };
        assert_eq!(decode_comment(&comment).unwrap().as_ref(), "15...Qf2 16.Ne4! +/-\nNext");
    }

    #[test]
    fn nags() {
        let symbols = [1, 0x20, 0x8c];
        assert_eq!(
            decode_nags(&symbols, Player::White).collect::<Vec<_>>(),
            [Nag::Numeric(1), Nag::Numeric(32), Nag::Numeric(140)]
        );
        assert_eq!(
            decode_nags(&symbols, Player::Black).collect::<Vec<_>>(),
            [Nag::Numeric(1), Nag::Numeric(33), Nag::Numeric(140)]
        );
    }
}
