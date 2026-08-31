use std::{
    io::{self, BufRead, BufReader, Read},
    iter::Peekable,
};

use encoding_rs::WINDOWS_1252;

use super::*;

pub fn games<R: Read>(reader: R) -> impl Iterator<Item = io::Result<Result<Game>>> {
    GameShaped::new(reader).map(|chunk| {
        chunk.map(|chunk| {
            game.parse(chunk.text.as_str())
                .map_err(|error| Error::from(&chunk.text, chunk.line, error))
        })
    })
}

struct GameShaped<R: Read> {
    lines: Peekable<Lines<R>>,
}

impl<R: Read> GameShaped<R> {
    fn new(reader: R) -> Self {
        Self { lines: Lines::new(reader).peekable() }
    }
}

impl<R: Read> Iterator for GameShaped<R> {
    type Item = io::Result<Chunk>;

    fn next(&mut self) -> Option<Self::Item> {
        game_shaped(&mut self.lines).transpose()
    }
}

struct Lines<R: Read> {
    reader: BufReader<R>,
    bytes: Vec<u8>,
    line: usize,
}

impl<R: Read> Lines<R> {
    fn new(reader: R) -> Self {
        Self { reader: BufReader::new(reader), bytes: Vec::new(), line: 0 }
    }
}

impl<R: Read> Iterator for Lines<R> {
    type Item = io::Result<SourceLine>;

    fn next(&mut self) -> Option<Self::Item> {
        self.bytes.clear();
        match self.reader.read_until(b'\n', &mut self.bytes) {
            Ok(0) => None,
            Ok(_) => {
                self.line += 1;
                Some(Ok(SourceLine { number: self.line, text: decode_line(&self.bytes) }))
            }
            Err(error) => Some(Err(error)),
        }
    }
}

struct SourceLine {
    number: usize,
    text: String,
}

struct Chunk {
    line: usize,
    text: String,
}

fn decode_line(bytes: &[u8]) -> String {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);

    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (text, _, _) = WINDOWS_1252.decode(bytes);
            text.into_owned()
        }
    }
}

fn game_shaped<I>(lines: &mut Peekable<I>) -> io::Result<Option<Chunk>>
where
    I: Iterator<Item = io::Result<SourceLine>>,
{
    let Some(line) = lines.next().transpose()? else {
        return Ok(None);
    };
    let first_line = line.number;
    let mut buffer = Buffer::new(line.text);

    loop {
        let next = match lines.peek() {
            Some(Ok(next)) => Some(next.text.as_str()),
            Some(Err(_)) | None => None,
        };
        if buffer.is_complete(next) {
            return Ok(Some(Chunk { line: first_line, text: buffer.take() }));
        }

        let Some(line) = lines.next().transpose()? else {
            return Ok(Some(Chunk { line: first_line, text: buffer.take() }));
        };
        buffer.push(line.text);
    }
}

struct Buffer {
    text: String,
    in_comment: bool,
    movetext: bool,
}

impl Buffer {
    fn new(line: String) -> Self {
        let mut buffer = Self { text: String::new(), in_comment: false, movetext: false };
        buffer.push(line);
        buffer
    }

    fn push(&mut self, line: String) {
        self.text.push_str(&line);
        self.text.push('\n');

        let line = if self.in_comment { line.as_str() } else { strip_tags(&line) };
        for c in line.chars() {
            if self.in_comment {
                if c == '}' {
                    self.in_comment = false;
                }
            } else {
                match c {
                    '{' => {
                        self.movetext = true;
                        self.in_comment = true;
                    }
                    ';' => {
                        self.movetext = true;
                        break;
                    }
                    c if c.is_whitespace() => {}
                    _ => self.movetext = true,
                }
            }
        }
    }

    fn is_game_shaped(&self) -> bool {
        self.movetext && !self.in_comment
    }

    fn is_complete(&self, next: Option<&str>) -> bool {
        self.is_game_shaped() && next.is_none_or(tag_start_ok)
    }

    fn take(self) -> String {
        self.text
    }
}
