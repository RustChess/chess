//! PGN format

use std::{
    fmt,
    io::{self, BufRead, BufReader, Read},
    iter::Peekable,
};

use encoding_rs::WINDOWS_1252;

use super::{StrInput as Input, prelude::*, san};

// https://www.chessprogramming.org/Portable_Game_Notation
// https://www.saremba.de/chessgml/standards/pgn/pgn-complete.htm
// https://github.com/mliebelt/pgn-spec-commented
// https://github.com/mliebelt/pgn-spec-commented/blob/main/pgn-spec-supplement.md
//
// Arrows and coloured squares
// [%cal Gc2c3,Rc3d4] green arrow c2-c3, red arrow c3-d4
// [$csl Ra3,Ga4] a3 red, a4 green
// # insert mini board in move list
// https://chesstempo.com/manual/en/manual.html#pgnviewercommentannotations
//

pub struct Pgn;

pub fn game(input: &mut Input<'_>) -> ModalResult<Game> {
    delimited(
        multispace0,
        seq! {Game {
        tag_pairs: tag_pairs,
        line: line,
        outcome: outcome,
        }},
        multispace0,
    )
    .context(StrContext::Label("PGN game"))
    .parse_next(input)
}

pub fn games(input: &mut Input<'_>) -> ModalResult<Games> {
    repeat(0.., game).map(|games| Games { games }).parse_next(input)
}

pub fn read_games<R: Read>(reader: R) -> impl Iterator<Item = io::Result<ModalResult<Game>>> {
    GameShaped::new(reader).map(|chunk| {
        chunk.map(|chunk| {
            game.parse(chunk.as_str()).map_err(|error| ErrMode::Backtrack(error.into_inner()))
        })
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Games {
    pub games: Vec<Game>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub tag_pairs: Vec<TagPair>,
    pub line: Line,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub annotations: Vec<Annotation>,
    pub moves: Vec<Move>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub san: san::San,
    pub annotations: Vec<Annotation>,
    pub variations: Vec<Variation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variation {
    pub line: Line,
    pub annotations: Vec<Annotation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagPair {
    pub name: Tag,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tag {
    Event,
    Site,
    Date,
    Round,
    White,
    Black,
    Result,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Annotation {
    Nag(Nag),
    Comment(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nag {
    Numeric(u32),
    Symbol(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    White,
    Black,
    Draw,
    Unknown,
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for tag_pair in &self.tag_pairs {
            writeln!(f, "{tag_pair}")?;
        }
        if !self.tag_pairs.is_empty() {
            writeln!(f)?;
        }
        let mut wrap = Wrap::new(f);
        self.line.write(&mut wrap, 0)?;
        wrap.token(self.outcome)
    }
}

impl fmt::Display for Games {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, game) in self.games.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
                writeln!(f)?;
            }
            game.fmt(f)?;
        }
        Ok(())
    }
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
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        game_shaped(&mut self.lines).transpose()
    }
}

struct Lines<R: Read> {
    reader: BufReader<R>,
    bytes: Vec<u8>,
}

impl<R: Read> Lines<R> {
    fn new(reader: R) -> Self {
        Self { reader: BufReader::new(reader), bytes: Vec::new() }
    }
}

impl<R: Read> Iterator for Lines<R> {
    type Item = io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.bytes.clear();
        match self.reader.read_until(b'\n', &mut self.bytes) {
            Ok(0) => None,
            Ok(_) => Some(Ok(decode_line(&self.bytes))),
            Err(error) => Some(Err(error)),
        }
    }
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

fn game_shaped<I>(lines: &mut Peekable<I>) -> io::Result<Option<String>>
where
    I: Iterator<Item = io::Result<String>>,
{
    let Some(line) = lines.next().transpose()? else {
        return Ok(None);
    };
    let mut buffer = Buffer::new(line);

    loop {
        let next = match lines.peek() {
            Some(Ok(next)) => Some(next.as_str()),
            Some(Err(_)) | None => None,
        };
        if buffer.is_complete(next) {
            return Ok(Some(buffer.take()));
        }

        let Some(line) = lines.next().transpose()? else {
            return Ok(Some(buffer.take()));
        };
        buffer.push(line);
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

        let line = if self.in_comment { line.as_str() } else { strip_tag_pairs(&line) };
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
        self.is_game_shaped() && next.is_none_or(tag_pair_start_ok)
    }

    fn take(self) -> String {
        self.text
    }
}

fn strip_tag_pairs(mut input: &str) -> &str {
    loop {
        let before = input;
        if tag_pair.parse_next(&mut input).is_err() {
            return before;
        }
    }
}

fn tag_pair_start_ok(mut input: &str) -> bool {
    tag_pair.parse_next(&mut input).is_ok()
}

impl fmt::Display for TagPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} \"{}\"]", self.name, escape_tag_value(&self.value))
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::Event => write!(f, "Event"),
            Tag::Site => write!(f, "Site"),
            Tag::Date => write!(f, "Date"),
            Tag::Round => write!(f, "Round"),
            Tag::White => write!(f, "White"),
            Tag::Black => write!(f, "Black"),
            Tag::Result => write!(f, "Result"),
            Tag::Other(tag) => write!(f, "{tag}"),
        }
    }
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Annotation::Nag(nag) => write!(f, "{nag}"),
            Annotation::Comment(comment) => write!(f, "{{{comment}}}"),
        }
    }
}

impl fmt::Display for Nag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Nag::Numeric(nag) => write!(f, "${nag}"),
            Nag::Symbol(nag) => write!(f, "{nag}"),
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::White => write!(f, "1-0"),
            Outcome::Black => write!(f, "0-1"),
            Outcome::Draw => write!(f, "1/2-1/2"),
            Outcome::Unknown => write!(f, "*"),
        }
    }
}

impl Line {
    fn write(&self, wrap: &mut Wrap<'_, '_>, first_ply: usize) -> fmt::Result {
        let mut ply = first_ply;
        for annotation in &self.annotations {
            wrap.token(annotation)?;
        }
        for (index, play) in self.moves.iter().enumerate() {
            if should_write_move_number(ply, index, &self.moves) {
                wrap.token(MoveNumber(ply))?;
                wrap.token(play.san)?;
            } else {
                wrap.token(play.san)?;
            }
            play.write_tail(wrap, ply)?;
            ply += 1;
        }
        Ok(())
    }
}

impl Move {
    fn write_tail(&self, wrap: &mut Wrap<'_, '_>, ply: usize) -> fmt::Result {
        for annotation in &self.annotations {
            wrap.token(annotation)?;
        }
        write_variations(&self.variations, wrap, ply)?;
        Ok(())
    }
}

fn write_variations(variations: &[Variation], wrap: &mut Wrap<'_, '_>, ply: usize) -> fmt::Result {
    for variation in variations {
        wrap.open("(")?;
        variation.line.write(wrap, ply)?;
        wrap.close(")")?;
        for annotation in &variation.annotations {
            wrap.token(annotation)?;
        }
    }
    Ok(())
}

struct MoveNumber(usize);

impl fmt::Display for MoveNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ply = self.0;
        let number = ply / 2 + 1;
        if ply.is_multiple_of(2) { write!(f, "{number}.") } else { write!(f, "{number}...") }
    }
}

struct Wrap<'a, 'b> {
    f: &'a mut fmt::Formatter<'b>,
    line: usize,
    glued: bool,
}

impl<'a, 'b> Wrap<'a, 'b> {
    const WIDTH: usize = 80;

    fn new(f: &'a mut fmt::Formatter<'b>) -> Self {
        Self { f, line: 0, glued: false }
    }

    fn token(&mut self, token: impl fmt::Display) -> fmt::Result {
        let token = token.to_string();
        let separator = usize::from(self.line > 0 && !self.glued);
        if self.line > 0 && self.line + separator + token.len() > Self::WIDTH {
            writeln!(self.f)?;
            self.line = 0;
            self.glued = false;
        } else if self.line > 0 && !self.glued {
            write!(self.f, " ")?;
            self.line += 1;
        }
        write!(self.f, "{token}")?;
        self.line += token.len();
        self.glued = false;
        Ok(())
    }

    fn open(&mut self, token: &str) -> fmt::Result {
        self.token(token)?;
        self.glued = true;
        Ok(())
    }

    fn close(&mut self, token: &str) -> fmt::Result {
        write!(self.f, "{token}")?;
        self.line += token.len();
        self.glued = false;
        Ok(())
    }
}

fn should_write_move_number(ply: usize, index: usize, moves: &[Move]) -> bool {
    ply.is_multiple_of(2)
        || index == 0
        || moves[index - 1].has_intervening_annotation_or_variation()
}

impl Move {
    fn has_intervening_annotation_or_variation(&self) -> bool {
        !self.annotations.is_empty() || !self.variations.is_empty()
    }
}

fn escape_tag_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn tag_pairs(input: &mut Input<'_>) -> ModalResult<Vec<TagPair>> {
    repeat(0.., tag_pair).parse_next(input)
}

pub fn tag_pair(input: &mut Input<'_>) -> ModalResult<TagPair> {
    delimited(
        multispace0,
        delimited(
            ('[', multispace0),
            separated_pair(tag_name, multispace1, tag_value),
            (multispace0, ']'),
        ),
        multispace0,
    )
    .map(|(name, value)| TagPair { name, value })
    .context(StrContext::Label("PGN tag pair"))
    .parse_next(input)
}

fn line(input: &mut Input<'_>) -> ModalResult<Line> {
    seq! {Line {
        annotations: annotations,
        moves: repeat(0.., parse_move),
    }}
    .parse_next(input)
}

fn parse_move(input: &mut Input<'_>) -> ModalResult<Move> {
    preceded(
        (multispace0, opt(skip_move_number), multispace0),
        seq! {Move {
            san: san::san.context(StrContext::Label("PGN move")),
            annotations: annotations,
            variations: variations,
        }},
    )
    .parse_next(input)
}

fn variations(input: &mut Input<'_>) -> ModalResult<Vec<Variation>> {
    repeat(0.., variation).parse_next(input)
}

fn variation(input: &mut Input<'_>) -> ModalResult<Variation> {
    seq! {Variation {
        line: preceded(multispace0, nested_line),
        annotations: annotations,
    }}
    .parse_next(input)
}

fn nested_line(input: &mut Input<'_>) -> ModalResult<Line> {
    delimited(('(', multispace0), line, (multispace0, ')'))
        .context(StrContext::Label("PGN variation"))
        .parse_next(input)
}

fn annotations(input: &mut Input<'_>) -> ModalResult<Vec<Annotation>> {
    repeat(0.., preceded(multispace0, annotation)).parse_next(input)
}

fn annotation(input: &mut Input<'_>) -> ModalResult<Annotation> {
    alt((
        bracket_comment.map(Annotation::Comment),
        semicolon_comment.map(Annotation::Comment),
        numeric_nag.map(Annotation::Nag),
        symbol_nag.map(Annotation::Nag),
    ))
    .parse_next(input)
}

// skip because it parses a valid 1. or 2... etc., but doesn't return it.
fn skip_move_number(input: &mut Input<'_>) -> ModalResult<()> {
    (dec_uint::<_, u32, _>, alt(("...", "."))).value(()).parse_next(input)
}

fn tag_name(input: &mut Input<'_>) -> ModalResult<Tag> {
    use Tag::*;

    take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
        .map(|name: &str| match name {
            "Event" => Event,
            "Site" => Site,
            "Date" => Date,
            "Round" => Round,
            "White" => White,
            "Black" => Black,
            "Result" => Result,
            _ => Other(name.to_string()),
        })
        .parse_next(input)
}

fn tag_value(input: &mut Input<'_>) -> ModalResult<String> {
    delimited(
        '"',
        repeat(0.., tag_value_char).fold(String::new, |mut value, c| {
            value.push(c);
            value
        }),
        '"',
    )
    .parse_next(input)
}

fn tag_value_char(input: &mut Input<'_>) -> ModalResult<char> {
    alt((preceded('\\', any), none_of(['"', '\\']))).parse_next(input)
}

fn numeric_nag(input: &mut Input<'_>) -> ModalResult<Nag> {
    preceded('$', dec_uint).map(Nag::Numeric).parse_next(input)
}

fn symbol_nag(input: &mut Input<'_>) -> ModalResult<Nag> {
    alt(("!!", "!?", "?!", "??", "!", "?"))
        .map(|nag: &str| Nag::Symbol(nag.to_string()))
        .parse_next(input)
}

fn bracket_comment(input: &mut Input<'_>) -> ModalResult<String> {
    preceded('{', terminated(take_till(0.., '}'), '}'))
        .map(|comment: &str| comment.trim().to_string())
        .context(StrContext::Label("PGN comment"))
        .parse_next(input)
}

fn semicolon_comment(input: &mut Input<'_>) -> ModalResult<String> {
    preceded(';', take_till(0.., '\n'))
        .map(|comment: &str| comment.trim().to_string())
        .parse_next(input)
}

fn outcome(input: &mut Input<'_>) -> ModalResult<Outcome> {
    use Outcome::*;

    preceded(
        multispace0,
        alt(("1/2-1/2".value(Draw), "1-0".value(White), "0-1".value(Black), "*".value(Unknown))),
    )
    .context(StrContext::Label("PGN outcome"))
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use crate::{
        formats::san,
        position::{File, Role, Square},
    };

    use super::*;

    #[test]
    fn parses_game() {
        let pgn = r#"
            [Event "Casual Game"]
            [Site "Berlin GER"]
            [Date "1852.??.??"]
            [Round "?"]
            [White "Adolf Anderssen"]
            [Black "Jean Dufresne"]
            [Result "1-0"]

            1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 {Italian Game} 1-0
        "#;

        let game = game.parse(pgn).unwrap();
        assert_eq!(game.tag_pairs.len(), 7);
        assert_eq!(game.tag_pairs[0].name, Tag::Event);
        assert_eq!(game.tag_pairs[0].value, "Casual Game");
        assert_eq!(game.line.moves.len(), 6);
        assert_eq!(game.outcome, Outcome::White);
        assert_eq!(
            game.line.moves[5].annotations,
            vec![Annotation::Comment("Italian Game".to_string())]
        );
    }

    #[test]
    fn parses_variation_and_nags() {
        let pgn = r#"[Event "x"]

1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *
"#;

        let game = game.parse(pgn).unwrap();
        let e4 = &game.line.moves[0];
        assert_eq!(
            e4.san.play,
            san::Move::Normal {
                role: Role::Pawn,
                file: None,
                rank: None,
                capture: false,
                to: Square::E4,
                promotion: None,
            }
        );
        assert_eq!(e4.annotations, vec![Annotation::Nag(Nag::Symbol("!".to_string()))]);

        let e5 = &game.line.moves[1];
        assert_eq!(e5.annotations, vec![Annotation::Nag(Nag::Numeric(1))]);
        assert_eq!(e5.variations.len(), 1);
        assert_eq!(
            e5.variations[0].line.moves[0].annotations,
            vec![Annotation::Comment("Sicilian".to_string())]
        );
        assert_eq!(game.outcome, Outcome::Unknown);
    }

    #[test]
    fn parses_annotations_between_variations() {
        let pgn = r#"[Event "x"]

1. e4 e5 {A} (1... c5) {B} (1... e6) {C} 2. Nf3 *
"#;

        let game = game.parse(pgn).unwrap();
        let e5 = &game.line.moves[1];
        assert_eq!(e5.annotations, vec![Annotation::Comment("A".to_string())]);
        assert_eq!(e5.variations.len(), 2);
        assert_eq!(e5.variations[0].annotations, vec![Annotation::Comment("B".to_string())]);
        assert_eq!(e5.variations[1].annotations, vec![Annotation::Comment("C".to_string())]);
    }

    #[test]
    fn parses_san_in_moves() {
        let game = game.parse(r#"[Event "x"] 1. exd8=Q# *"#).unwrap();
        assert_eq!(
            game.line.moves[0].san,
            san::San {
                play: san::Move::Normal {
                    role: Role::Pawn,
                    file: Some(File::E),
                    rank: None,
                    capture: true,
                    to: Square::D8,
                    promotion: Some(Role::Queen),
                },
                check: Some(san::Check::Checkmate),
            }
        );
    }

    #[test]
    fn displays_game() {
        let game = game.parse(r#"[Event "x"] 1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *"#).unwrap();
        assert_eq!(
            game.to_string(),
            "[Event \"x\"]\n\n1. e4 ! 1... e5 $1 (1... c5 {Sicilian}) 2. Nf3 *"
        );
    }

    #[test]
    fn parses_and_displays_games() {
        let parsed = games
            .parse(
                r#"[Event "a"] 1. e4 *

[Event "b"] 1. d4 *
"#,
            )
            .unwrap();
        assert_eq!(parsed.games.len(), 2);
        assert_eq!(parsed.to_string(), "[Event \"a\"]\n\n1. e4 *\n\n[Event \"b\"]\n\n1. d4 *");
    }

    #[test]
    fn reads_games_one_at_a_time() {
        let input = b"[Event \"a\"]\n1. e4 *\n\n[Event \"b\"]\n1. d4 *\n";
        let games = read_games(&input[..]).map(|game| game.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].tag_pairs[0].value, "a");
        assert_eq!(games[1].tag_pairs[0].value, "b");
    }

    #[test]
    fn reader_does_not_split_inside_comment() {
        let input = b"[Event \"a\"]\n1. e4 {\n[not a tag]\n} *\n\n[Event \"b\"]\n1. d4 *\n";
        let games = read_games(&input[..]).map(|game| game.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(games.len(), 2);
        assert_eq!(
            games[0].line.moves[0].annotations,
            vec![Annotation::Comment("[not a tag]".to_string())]
        );
    }

    #[test]
    fn reader_handles_movetext_after_tag_on_same_line() {
        let input = b"[Event \"a\"] 1. e4 *\n\n[Event \"b\"] 1. d4 *\n";
        let games = read_games(&input[..]).map(|game| game.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].tag_pairs[0].value, "a");
        assert_eq!(games[1].tag_pairs[0].value, "b");
    }
}
