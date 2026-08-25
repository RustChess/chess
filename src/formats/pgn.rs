//! PGN format

use std::{
    fmt,
    io::{self, BufRead, BufReader, Read},
    iter::Peekable,
};

use encoding_rs::WINDOWS_1252;

use crate::Position;
use crate::game::{self, Command, Id, Nag, Outcome, Tag, TagPair, Text};

use super::{StrInput as Input, fen, prelude::*, san};

// https://www.chessprogramming.org/Portable_Game_Notation
// https://www.saremba.de/chessgml/standards/pgn/pgn-complete.htm
// https://github.com/mliebelt/pgn-spec-commented
// https://github.com/mliebelt/pgn-spec-commented/blob/main/pgn-spec-supplement.md
//
// Arrows and coloured squares
// [%cal Gc2c3,Rc3d4] green arrow c2-c3, red arrow c3-d4
// [%csl Ra3,Ga4] a3 red, a4 green
// # insert mini board in move list
// https://chesstempo.com/manual/en/manual.html#pgnviewercommentannotations
//

pub fn parse_game(input: &mut Input<'_>) -> ModalResult<Game> {
    delimited(
        multispace0,
        seq! {Game {
        tag_pairs: tag_pairs,
        intro: comments,
        moves: repeat(0.., parse_move),
        outcome: outcome,
        }},
        multispace0,
    )
    .context(StrContext::Label("PGN game"))
    .parse_next(input)
}

pub fn games(input: &mut Input<'_>) -> ModalResult<Games> {
    repeat(0.., parse_game).map(|games| Games { games }).parse_next(input)
}

pub fn read_games<R: Read>(reader: R) -> impl Iterator<Item = io::Result<ModalResult<Game>>> {
    GameShaped::new(reader).map(|chunk| {
        chunk.map(|chunk| {
            parse_game.parse(chunk.as_str()).map_err(|error| ErrMode::Backtrack(error.into_inner()))
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
    pub intro: Option<Text>,
    pub moves: Vec<Move>,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variation {
    pub intro: Option<Text>,
    pub moves: Vec<Move>,
    pub outro: Option<Text>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub san: san::San,
    pub comment: Option<Text>,
    pub annotations: Vec<Annotation>,
    pub variations: Vec<Variation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Annotation {
    Nag(Nag),
    Command(Command),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SAN error: {0}")]
    San(#[from] san::Error),
    #[error("game error: {0}")]
    Game(#[from] game::Error),
    #[error("invalid FEN: {0}")]
    Fen(String),
    #[error("empty PGN variation of {} {san}", MoveNumber(*ply))]
    EmptyVariation { ply: usize, san: san::San },
}

impl TryFrom<Game> for game::Game {
    type Error = Error;

    fn try_from(pgn: Game) -> Result<Self, Self::Error> {
        let position = position(&pgn.tag_pairs)?;
        let mut game = game::Game::new(position);
        game.tags = pgn.tag_pairs;
        game.intro = pgn.intro;
        game.outcome = pgn.outcome;

        convert_moves(&mut game, None, 0, pgn.moves)?;

        Ok(game)
    }
}

fn position(tag_pairs: &[TagPair]) -> Result<Position, Error> {
    if let Some(fen) = tag_pairs.iter().find(|tag_pair| tag_pair.name == Tag::Fen) {
        let unvalidated = fen::position_fen
            .parse(fen.value.as_str())
            .map_err(|_| Error::Fen(fen.value.clone()))?;
        Position::new(unvalidated).map_err(|_| Error::Fen(fen.value.clone()))
    } else {
        Ok(Position::standard())
    }
}

fn convert_moves(
    game: &mut game::Game,
    mut previous: Option<Id>,
    mut ply: usize,
    moves: Vec<Move>,
) -> Result<(), Error> {
    for pgn_move in moves {
        let mut lines = game.lines_mut_at(previous.clone()).expect("previous play exists");
        let play = pgn_move.san.resolve(lines.state())?;
        let id = lines.push(play)?;

        {
            let mut play = game.play_mut(id.clone()).expect("inserted play exists");
            play.meta.comment = pgn_move.comment;
            for annotation in pgn_move.annotations {
                match annotation {
                    Annotation::Nag(nag) => play.meta.nags.push(nag),
                    Annotation::Command(command) => play.meta.commands.push(command),
                }
            }
        }

        for variation in pgn_move.variations {
            convert_variation(game, previous.clone(), ply, pgn_move.san, variation)?;
        }

        previous = Some(id);
        ply += 1;
    }

    Ok(())
}

fn convert_variation(
    game: &mut game::Game,
    previous: Option<Id>,
    ply: usize,
    after: san::San,
    variation: Variation,
) -> Result<(), Error> {
    let Some((first, rest)) = variation.moves.split_first() else {
        return Err(Error::EmptyVariation { ply, san: after });
    };

    let mut lines = game.lines_mut_at(previous.clone()).expect("previous play exists");
    let play = first.san.resolve(lines.state())?;
    let id = lines.push(play)?;

    {
        let mut play = game.play_mut(id.clone()).expect("inserted play exists");
        play.meta.intro = variation.intro;
        play.meta.outro = variation.outro;
        play.meta.comment = first.comment.clone();
        for annotation in &first.annotations {
            match annotation {
                Annotation::Nag(nag) => play.meta.nags.push(nag.clone()),
                Annotation::Command(command) => play.meta.commands.push(command.clone()),
            }
        }
    }

    for variation in &first.variations {
        convert_variation(game, previous.clone(), ply, first.san, variation.clone())?;
    }
    convert_moves(game, Some(id), ply + 1, rest.to_vec())
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
        if let Some(intro) = &self.intro {
            wrap.token(intro)?;
        }
        write_moves(&self.moves, &mut wrap, 0)?;
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
            Tag::Fen => write!(f, "FEN"),
            Tag::SetUp => write!(f, "SetUp"),
            Tag::Other(tag) => write!(f, "{tag}"),
        }
    }
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Annotation::Nag(nag) => write!(f, "{nag}"),
            Annotation::Command(command) => write!(f, "{command}"),
        }
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{}}}", self.as_ref())
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[%{}", self.command)?;
        for (index, parameter) in self.parameters.iter().enumerate() {
            if index == 0 {
                write!(f, " ")?;
            } else {
                write!(f, ",")?;
            }
            write!(f, "{parameter}")?;
        }
        write!(f, "]")
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

fn write_moves(moves: &[Move], wrap: &mut Wrap<'_, '_>, first_ply: usize) -> fmt::Result {
    for (ply, (index, play)) in (first_ply..).zip(moves.iter().enumerate()) {
        if should_write_move_number(ply, index, moves) {
            wrap.token(MoveNumber(ply))?;
            wrap.token(play.san)?;
        } else {
            wrap.token(play.san)?;
        }
        play.write_tail(wrap, ply)?;
    }
    Ok(())
}

impl Move {
    fn write_tail(&self, wrap: &mut Wrap<'_, '_>, ply: usize) -> fmt::Result {
        let mut commands = Vec::new();
        for annotation in &self.annotations {
            match annotation {
                Annotation::Nag(_) => wrap.token(annotation)?,
                Annotation::Command(command) => commands.push(command.clone()),
            }
        }
        if !commands.is_empty() || self.comment.is_some() {
            wrap.token(MoveComment { commands, text: self.comment.clone() })?;
        }
        write_variations(&self.variations, wrap, ply)?;
        Ok(())
    }
}

fn write_variations(variations: &[Variation], wrap: &mut Wrap<'_, '_>, ply: usize) -> fmt::Result {
    for variation in variations {
        wrap.open("(")?;
        if let Some(intro) = &variation.intro {
            wrap.token(intro)?;
        }
        write_moves(&variation.moves, wrap, ply)?;
        wrap.close(")")?;
        if let Some(outro) = &variation.outro {
            wrap.token(outro)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct MoveNumber(usize);

struct MoveComment {
    commands: Vec<Command>,
    text: Option<Text>,
}

impl fmt::Display for MoveComment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for (index, command) in self.commands.iter().enumerate() {
            if index > 0 {
                write!(f, " ")?;
            }
            write!(f, "{command}")?;
        }
        if let Some(text) = &self.text {
            if !self.commands.is_empty() {
                write!(f, " ")?;
            }
            write!(f, "{}", text)?;
        }
        write!(f, "}}")
    }
}

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
        !self.annotations.is_empty() || self.comment.is_some() || !self.variations.is_empty()
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

fn parse_move(input: &mut Input<'_>) -> ModalResult<Move> {
    preceded(
        (multispace0, opt(skip_move_number), multispace0),
        (san::san.context(StrContext::Label("PGN move")), tail, variations).map(
            |(san, (comment, annotations), variations)| Move {
                san,
                comment,
                annotations,
                variations,
            },
        ),
    )
    .parse_next(input)
}

fn variations(input: &mut Input<'_>) -> ModalResult<Vec<Variation>> {
    repeat(0.., variation).parse_next(input)
}

// ({intro} 1... c5 {move comment}) {outro}
fn variation(input: &mut Input<'_>) -> ModalResult<Variation> {
    let mut variation = preceded(
        multispace0,
        delimited(
            ('(', multispace0),
            seq! {Variation {
                intro: comments,
                moves: repeat(0.., parse_move),
                outro: ().value(None),
            }},
            (multispace0, ')'),
        ),
    )
    .context(StrContext::Label("PGN variation"))
    .parse_next(input)?;
    variation.outro = comments(input)?;
    Ok(variation)
}

fn tail(input: &mut Input<'_>) -> ModalResult<(Option<Text>, Vec<Annotation>)> {
    repeat(0.., preceded(multispace0, tail_item))
        .fold(
            || (None, Vec::new()),
            |(mut comment, mut annotations), item| {
                match item {
                    Tail::Comment(MoveComment { commands, text }) => {
                        annotations.extend(commands.into_iter().map(Annotation::Command));
                        if let Some(text) = text {
                            merge_comments(&mut comment, text);
                        }
                    }
                    Tail::Annotation(annotation) => annotations.push(annotation),
                }
                (comment, annotations)
            },
        )
        .parse_next(input)
}

enum Tail {
    Comment(MoveComment),
    Annotation(Annotation),
}

fn tail_item(input: &mut Input<'_>) -> ModalResult<Tail> {
    alt((
        move_comment.map(Tail::Comment),
        numeric_nag.map(Annotation::Nag).map(Tail::Annotation),
        symbol_nag.map(Annotation::Nag).map(Tail::Annotation),
    ))
    .parse_next(input)
}

fn comments(input: &mut Input<'_>) -> ModalResult<Option<Text>> {
    repeat(0.., preceded(multispace0, comment))
        .fold(
            || None,
            |mut comments, comment| {
                if let Some(comment) = comment {
                    merge_comments(&mut comments, comment);
                }
                comments
            },
        )
        .parse_next(input)
}

// skip because it parses a valid 1. or 2... etc., but doesn't return it.
fn skip_move_number(input: &mut Input<'_>) -> ModalResult<()> {
    (dec_uint::<_, u32, _>, alt(("...", "."))).value(()).parse_next(input)
}

fn tag_name(input: &mut Input<'_>) -> ModalResult<Tag> {
    use Tag::*;

    name.map(|name: &str| match name {
        "Event" => Event,
        "Site" => Site,
        "Date" => Date,
        "Round" => Round,
        "White" => White,
        "Black" => Black,
        "Result" => Result,
        "FEN" => Fen,
        "SetUp" => SetUp,
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

fn name<'a>(input: &mut Input<'a>) -> ModalResult<&'a str> {
    take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_').parse_next(input)
}

fn numeric_nag(input: &mut Input<'_>) -> ModalResult<Nag> {
    preceded('$', dec_uint).map(Nag::Numeric).parse_next(input)
}

fn symbol_nag(input: &mut Input<'_>) -> ModalResult<Nag> {
    alt(("!!", "!?", "?!", "??", "!", "?"))
        .map(|nag: &str| Nag::Symbol(nag.to_string()))
        .parse_next(input)
}

fn comment(input: &mut Input<'_>) -> ModalResult<Option<Text>> {
    alt((bracket_comment, semicolon_comment))
        .map(Text::new)
        .context(StrContext::Label("PGN comment"))
        .parse_next(input)
}

fn move_comment(input: &mut Input<'_>) -> ModalResult<MoveComment> {
    alt((bracket_comment, semicolon_comment))
        .map(split_comment)
        .context(StrContext::Label("PGN comment"))
        .parse_next(input)
}

fn bracket_comment(input: &mut Input<'_>) -> ModalResult<String> {
    preceded('{', terminated(take_till(0.., '}'), '}')).map(ToString::to_string).parse_next(input)
}

fn semicolon_comment(input: &mut Input<'_>) -> ModalResult<String> {
    preceded(';', take_till(0.., '\n')).map(ToString::to_string).parse_next(input)
}

fn split_comment(raw: String) -> MoveComment {
    let mut commands = Vec::new();
    let mut comment = String::new();
    let mut rest: Input<'_> = raw.as_str();

    while let Some(start) = rest.find("[%") {
        comment.push_str(&rest[..start]);
        rest = &rest[start..];

        let mut candidate: Input<'_> = rest;
        if let Ok(command) = command.parse_next(&mut candidate) {
            commands.push(command);
            rest = candidate;
        } else {
            if let Some((invalid_command, next)) = rest.split_once(']') {
                comment.push_str(invalid_command);
                comment.push(']');
                rest = next;
            } else {
                comment.push_str(rest);
                rest = "";
            }
        }
    }

    comment.push_str(rest);
    MoveComment { commands, text: Text::new(comment) }
}

fn command(input: &mut Input<'_>) -> ModalResult<Command> {
    delimited("[%", (name, opt(preceded(space1, parameters))), (space0, ']'))
        .map(|(command, parameters): (&str, Option<Vec<String>>)| Command {
            command: command.to_string(),
            parameters: parameters.unwrap_or_default(),
        })
        .parse_next(input)
}

fn parameters(input: &mut Input<'_>) -> ModalResult<Vec<String>> {
    separated(0.., parameter, ',').parse_next(input)
}

fn parameter(input: &mut Input<'_>) -> ModalResult<String> {
    alt((quoted_parameter, unquoted_parameter)).parse_next(input)
}

fn quoted_parameter(input: &mut Input<'_>) -> ModalResult<String> {
    // Command parameters are a loose extension. Escaped quotes are not handled here, and
    // `.take()` keeps the surrounding quotes so display can write the raw parameter back.
    delimited('"', take_till(0.., '"'), '"').take().map(ToString::to_string).parse_next(input)
}

fn unquoted_parameter(input: &mut Input<'_>) -> ModalResult<String> {
    take_till(1.., [',', ']']).map(|parameter: &str| parameter.trim().to_string()).parse_next(input)
}

fn merge_comments(into: &mut Option<Text>, comment: Text) {
    match into {
        Some(into) => into.merge(&comment),
        None => *into = Some(comment),
    }
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

    fn comment(text: &str) -> Text {
        Text::new(text).unwrap()
    }

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

        let game = parse_game.parse(pgn).unwrap();
        assert_eq!(game.tag_pairs.len(), 7);
        assert_eq!(game.tag_pairs[0].name, Tag::Event);
        assert_eq!(game.tag_pairs[0].value, "Casual Game");
        assert_eq!(game.moves.len(), 6);
        assert_eq!(game.outcome, Outcome::White);
        assert_eq!(game.moves[5].comment, Some(comment("Italian Game")));
    }

    #[test]
    fn parses_variation_and_nags() {
        let pgn = r#"[Event "x"]

1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *
"#;

        let game = parse_game.parse(pgn).unwrap();
        let e4 = &game.moves[0];
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

        let e5 = &game.moves[1];
        assert_eq!(e5.annotations, vec![Annotation::Nag(Nag::Numeric(1))]);
        assert_eq!(e5.variations.len(), 1);
        assert_eq!(e5.variations[0].moves[0].comment, Some(comment("Sicilian")));
        assert_eq!(game.outcome, Outcome::Unknown);
    }

    #[test]
    fn converts_to_game() {
        let pgn = parse_game
            .parse(
                r#"[Event "x"]

1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *
"#,
            )
            .unwrap();
        let game = game::Game::try_from(pgn).unwrap();

        assert_eq!(game.tags.len(), 1);
        assert_eq!(game.lines().len(), 1);

        let e4_id = game.lines().lines()[0].clone();
        let e4 = game.play(e4_id).unwrap();
        assert_eq!(e4.play().to, Square::E4);
        assert_eq!(e4.meta.nags, vec![Nag::Symbol("!".to_string())]);
        let e4_lines = e4.lines();
        assert_eq!(e4_lines.len(), 2);

        let e5 = e4_lines.get(0).unwrap();
        assert_eq!(e5.play().to, Square::E5);
        assert_eq!(e5.meta.nags, vec![Nag::Numeric(1)]);

        let c5 = e4_lines.get(1).unwrap();
        assert_eq!(c5.play().to, Square::C5);
        assert_eq!(c5.meta.comment, Some(comment("Sicilian")));
    }

    #[test]
    fn converts_fen_game() {
        let pgn = parse_game
            .parse(
                r#"[FEN "8/8/8/8/8/8/4P3/4K3 w - - 0 1"]

1. e4 *
"#,
            )
            .unwrap();
        let game = game::Game::try_from(pgn).unwrap();

        let lines = game.lines();
        let e4 = lines.get(0).unwrap();
        assert_eq!(e4.play().from, Square::E2);
        assert_eq!(e4.play().to, Square::E4);
    }

    #[test]
    fn parses_annotations_between_variations() {
        let pgn = r#"[Event "x"]

1. e4 e5 {A} (1... c5) {B} (1... e6) {C} 2. Nf3 *
"#;

        let game = parse_game.parse(pgn).unwrap();
        let e5 = &game.moves[1];
        assert_eq!(e5.comment, Some(comment("A")));
        assert_eq!(e5.variations.len(), 2);
        assert_eq!(e5.variations[0].outro, Some(comment("B")));
        assert_eq!(e5.variations[1].outro, Some(comment("C")));
        assert_eq!(
            game.to_string(),
            r#"[Event "x"]

1. e4 e5 {A} (1... c5) {B} (1... e6) {C} 2. Nf3 *"#
        );
    }

    #[test]
    fn extracts_commands_from_move_comments() {
        let game = parse_game
            .parse(r#"[Event "x"] 1. e4 {[%cal Ge2e4] [%clk 0:14:49] Good move.} *"#)
            .unwrap();
        let e4 = &game.moves[0];
        assert_eq!(
            e4.annotations,
            vec![
                Annotation::Command(Command {
                    command: "cal".to_string(),
                    parameters: vec!["Ge2e4".to_string()],
                }),
                Annotation::Command(Command {
                    command: "clk".to_string(),
                    parameters: vec!["0:14:49".to_string()],
                }),
            ]
        );
        assert_eq!(e4.comment, Some(comment("Good move.")));
        assert_eq!(
            game.to_string(),
            r#"[Event "x"]

1. e4 {[%cal Ge2e4] [%clk 0:14:49] Good move.} *"#
        );
    }

    #[test]
    fn extracts_multi_parameter_commands() {
        let game = parse_game
            .parse(r#"[Event "x"] 1. e4 {[%tqu "En","find the move","","","e2e4","",10]} *"#)
            .unwrap();
        let e4 = &game.moves[0];
        assert_eq!(
            e4.annotations,
            vec![Annotation::Command(Command {
                command: "tqu".to_string(),
                parameters: vec![
                    r#""En""#.to_string(),
                    r#""find the move""#.to_string(),
                    r#""""#.to_string(),
                    r#""""#.to_string(),
                    r#""e2e4""#.to_string(),
                    r#""""#.to_string(),
                    "10".to_string(),
                ],
            })]
        );
        assert_eq!(
            game.to_string(),
            r#"[Event "x"]

1. e4 {[%tqu "En","find the move","","","e2e4","",10]} *"#
        );
    }

    #[test]
    fn extracts_commands_with_trailing_whitespace() {
        let game = parse_game.parse(r#"[Event "x"] 1. e4 {[%foo ]} *"#).unwrap();
        assert_eq!(
            game.moves[0].annotations,
            vec![Annotation::Command(Command { command: "foo".to_string(), parameters: vec![] })]
        );
        assert_eq!(
            game.to_string(),
            r#"[Event "x"]

1. e4 {[%foo]} *"#
        );
    }

    #[test]
    fn ignores_empty_comments() {
        let game = parse_game.parse(r#"[Event "x"] 1. e4 {} {   } e5 *"#).unwrap();
        assert_eq!(game.moves[0].comment, None);
        assert_eq!(game.moves[1].comment, None);
    }

    #[test]
    fn parses_san_in_moves() {
        let game = parse_game.parse(r#"[Event "x"] 1. exd8=Q# *"#).unwrap();
        assert_eq!(
            game.moves[0].san,
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
        let game =
            parse_game.parse(r#"[Event "x"] 1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *"#).unwrap();
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
        assert_eq!(games[0].moves[0].comment, Some(comment("[not a tag]")));
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
