//! PGN format

use std::{fmt, str};

use crate::{
    game::{Command, Nag, Outcome, Slot, Tag as OtherTag, Text},
    position::{Position, SupportedEnum, Unvalidated},
};

use super::{StrInput as Input, fen, prelude::*, san};

pub mod convert;
pub mod stream;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub tags: Vec<Tag>,
    pub start: Position<Unvalidated>,
    pub intro: Option<Comment>,
    pub moves: Vec<Move>,
    pub outcome: Outcome,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tag {
    Event(String),
    Site(String),
    Date(String),
    Round(String),
    White(String),
    Black(String),
    Outcome(Outcome),
    Fen(Position<Unvalidated>),
    SetUp(bool),
    Variant(String),
    Other(OtherTag),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("PGN error at line {line}, column {column}: {message}")]
pub struct Error {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub fn game(input: &mut Input<'_>) -> ModalResult<Game> {
    delimited(multispace0, (tags, comments, repeat(0.., parse_move), outcome), multispace0)
        .map(|(tags, intro, moves, outcome)| Game {
            start: start_position(&tags),
            tags,
            intro,
            moves,
            outcome,
        })
        .context(StrContext::Label("PGN game"))
        .parse_next(input)
}

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for tag in &self.tags {
            writeln!(f, "{tag}")?;
        }
        if !self.tags.is_empty() {
            writeln!(f)?;
        }
        let mut wrap = Wrap::<_, 80>::new(f);
        if let Some(intro) = &self.intro {
            wrap.token(intro)?;
        }
        write_moves(&self.moves, &mut wrap, self.start.first_ply(), Notation::San)?;
        wrap.token(self.outcome)
    }
}

impl str::FromStr for Game {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        game.parse(text).map_err(|error| Error::from(text, 1, error))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variation {
    pub intro: Option<Comment>,
    pub moves: Vec<Move>,
    pub outro: Option<Comment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub san: san::San,
    pub comment: Option<Comment>,
    pub annotations: Vec<Annotation>,
    pub variations: Vec<Variation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comment(Text);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Annotation {
    Nag(Nag),
    Command(Command),
}

impl Error {
    fn from(
        text: &str,
        first_line: usize,
        error: winnow::error::ParseError<Input<'_>, ContextError>,
    ) -> Self {
        let (line, column) = line_column(text, first_line, error.offset());
        Self { line, column, message: format!("{:?}", error.inner()) }
    }
}

fn line_column(input: &str, first_line: usize, offset: usize) -> (usize, usize) {
    let mut line = first_line;
    let mut column = 1;

    for (index, char) in input.char_indices() {
        if index >= offset {
            break;
        }
        if char == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

impl Game {
    pub fn movetext(&self) -> String {
        self.write_movetext(Notation::San)
    }

    pub fn figurine_movetext(&self) -> String {
        self.write_movetext(Notation::Figurine)
    }

    fn write_movetext(&self, notation: Notation) -> String {
        let mut movetext = String::new();
        let mut wrap = Wrap::<_, 0>::new(&mut movetext);
        if let Some(intro) = &self.intro {
            wrap.token(intro).expect("writing PGN movetext to string");
        }
        write_moves(&self.moves, &mut wrap, self.start.first_ply(), notation)
            .expect("writing PGN movetext to string");
        movetext
    }

    pub fn freestyle(&self) -> Result<bool, String> {
        let freestyle = match self.tag_variant()? {
            SupportedEnum::Chess => !self.start.castles().chess_compatible(),
            SupportedEnum::Freestyle => true,
        };

        Ok(freestyle)
    }

    pub fn tag_variant(&self) -> Result<SupportedEnum, String> {
        let mut variant = SupportedEnum::Chess;
        for tag in &self.tags {
            if let Tag::Variant(value) = tag {
                variant = SupportedEnum::from_tag(value)?;
            }
        }
        Ok(variant)
    }
}

fn strip_tags(mut input: &str) -> &str {
    loop {
        let before = input;
        if tag.parse_next(&mut input).is_err() {
            return before;
        }
    }
}

fn tag_start_ok(mut input: &str) -> bool {
    tag.parse_next(&mut input).is_ok()
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tag::Event(value) => write_tag(f, "Event", value),
            Tag::Site(value) => write_tag(f, "Site", value),
            Tag::Date(value) => write_tag(f, "Date", value),
            Tag::Round(value) => write_tag(f, "Round", value),
            Tag::White(value) => write_tag(f, "White", value),
            Tag::Black(value) => write_tag(f, "Black", value),
            Tag::Outcome(outcome) => write_tag(f, "Result", &outcome.to_string()),
            Tag::Fen(position) => write_tag(f, "FEN", &position.fen()),
            Tag::SetUp(setup) => write_tag(f, "SetUp", if *setup { "1" } else { "0" }),
            Tag::Variant(variant) => write_tag(f, "Variant", variant),
            Tag::Other(tag) => write_tag(f, tag.key.as_ref(), &tag.value),
        }
    }
}

impl SupportedEnum {
    fn from_tag(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "chess" | "standard" => Ok(Self::Chess),
            "chess960" | "fischerandom" | "fischer random" | "freestyle" => Ok(Self::Freestyle),
            _ => Err(value.to_string()),
        }
    }
}

fn write_tag(f: &mut fmt::Formatter<'_>, key: &str, value: &str) -> fmt::Result {
    fn escape_tag_value(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"")
    }

    write!(f, "[{} \"{}\"]", key, escape_tag_value(value))
}

fn start_position(tags: &[Tag]) -> Position<Unvalidated> {
    tags.iter()
        .rev()
        .find_map(|tag| match tag {
            Tag::Fen(position) => Some(*position),
            _ => None,
        })
        .unwrap_or_else(Position::chess)
}

impl fmt::Display for Annotation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Annotation::Nag(nag) => write!(f, "{nag}"),
            Annotation::Command(command) => write!(f, "{command}"),
        }
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

fn write_moves<W: fmt::Write + ?Sized, const WIDTH: usize>(
    moves: &[Move],
    wrap: &mut Wrap<'_, W, WIDTH>,
    first_ply: usize,
    notation: Notation,
) -> fmt::Result {
    for (ply, (index, play)) in (first_ply..).zip(moves.iter().enumerate()) {
        if should_write_move_number(ply, index, moves) {
            wrap.token(MoveNumber(ply))?;
        }
        match notation {
            Notation::San => wrap.token(play.san)?,
            Notation::Figurine => wrap.token(play.san.figurine())?,
        }
        play.write_tail(wrap, ply, notation)?;
    }
    Ok(())
}

impl Move {
    fn write_tail<W: fmt::Write + ?Sized, const WIDTH: usize>(
        &self,
        wrap: &mut Wrap<'_, W, WIDTH>,
        ply: usize,
        notation: Notation,
    ) -> fmt::Result {
        let mut commands = Vec::new();
        for annotation in &self.annotations {
            match annotation {
                Annotation::Nag(Nag::Symbol(_)) => wrap.suffix(annotation)?,
                Annotation::Nag(Nag::Numeric(_)) => wrap.token(annotation)?,
                Annotation::Command(command) => commands.push(command.clone()),
            }
        }
        if !commands.is_empty() || self.comment.is_some() {
            wrap.token(MoveComment { commands, text: self.comment.clone() })?;
        }
        write_variations(&self.variations, wrap, ply, notation)?;
        Ok(())
    }
}

fn write_variations<W: fmt::Write + ?Sized, const WIDTH: usize>(
    variations: &[Variation],
    wrap: &mut Wrap<'_, W, WIDTH>,
    ply: usize,
    notation: Notation,
) -> fmt::Result {
    for variation in variations {
        wrap.open("(")?;
        if let Some(intro) = &variation.intro {
            wrap.token(intro)?;
        }
        write_moves(&variation.moves, wrap, ply, notation)?;
        wrap.close(")")?;
        if let Some(outro) = &variation.outro {
            wrap.token(outro)?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Notation {
    San,
    Figurine,
}

#[derive(Debug)]
struct MoveNumber(usize);

struct MoveComment {
    commands: Vec<Command>,
    text: Option<Comment>,
}

impl Comment {
    fn merge(&mut self, comment: &Comment) {
        self.0.merge(&comment.0);
    }
}

impl From<Comment> for Text {
    fn from(comment: Comment) -> Self {
        comment.0
    }
}

impl fmt::Display for Comment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{}}}", self.0)
    }
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
            write!(f, "{}", text.0)?;
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

struct Wrap<'a, W: fmt::Write + ?Sized, const WIDTH: usize = 80> {
    f: &'a mut W,
    line: usize,
    glued: bool,
}

impl<'a, W: fmt::Write + ?Sized, const WIDTH: usize> Wrap<'a, W, WIDTH> {
    fn new(f: &'a mut W) -> Self {
        Self { f, line: 0, glued: false }
    }

    fn token(&mut self, token: impl fmt::Display) -> fmt::Result {
        let token = token.to_string();
        let separator = usize::from(self.line > 0 && !self.glued);
        if WIDTH > 0 && self.line > 0 && self.line + separator + token.len() > WIDTH {
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

    fn suffix(&mut self, token: impl fmt::Display) -> fmt::Result {
        let token = token.to_string();
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

pub fn tags(input: &mut Input<'_>) -> ModalResult<Vec<Tag>> {
    repeat(0.., tag).parse_next(input)
}

pub fn tag(input: &mut Input<'_>) -> ModalResult<Tag> {
    delimited(
        multispace0,
        delimited(
            ('[', multispace0),
            separated_pair(name, multispace1, tag_value),
            (multispace0, ']'),
        ),
        multispace0,
    )
    .verify_map(|(key, value)| tag_from_pair(key, value))
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
    repeat(0.., preceded(multispace0, variation)).parse_next(input)
}

// ({intro} 1... c5 {move comment}) {outro}
fn variation(input: &mut Input<'_>) -> ModalResult<Variation> {
    let mut variation = delimited(
        ('(', multispace0),
        seq! {Variation {
            intro: comments,
            moves: repeat(0.., parse_move),
            outro: ().value(None),
        }},
        (multispace0, ')'),
    )
    .context(StrContext::Label("PGN variation"))
    .parse_next(input)?;
    variation.outro = comments(input)?;
    Ok(variation)
}

fn tail(input: &mut Input<'_>) -> ModalResult<(Option<Comment>, Vec<Annotation>)> {
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

fn comments(input: &mut Input<'_>) -> ModalResult<Option<Comment>> {
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

fn tag_from_pair(key: Text, value: String) -> Option<Tag> {
    Some(match key.as_ref() {
        "Event" => Tag::Event(value),
        "Site" => Tag::Site(value),
        "Date" => Tag::Date(value),
        "Round" => Tag::Round(value),
        "White" => Tag::White(value),
        "Black" => Tag::Black(value),
        "Result" => Tag::Outcome(outcome.parse(value.as_str()).ok()?),
        "FEN" => Tag::Fen(fen::parse_position.parse(value.as_str()).ok()?),
        "SetUp" => Tag::SetUp(match value.as_str() {
            "0" => false,
            "1" => true,
            _ => return None,
        }),
        "Variant" => Tag::Variant(value),
        _ => Tag::Other(OtherTag { key, value }),
    })
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

fn name(input: &mut Input<'_>) -> ModalResult<Text> {
    take_while(1.., |c: char| c.is_ascii_alphanumeric() || c == '_')
        .verify_map(Text::new)
        .parse_next(input)
}

fn numeric_nag(input: &mut Input<'_>) -> ModalResult<Nag> {
    preceded('$', dec_uint).map(Nag::Numeric).parse_next(input)
}

fn symbol_nag(input: &mut Input<'_>) -> ModalResult<Nag> {
    alt(("!!", "!?", "?!", "??", "!", "?"))
        .map(|nag: &str| Nag::Symbol(nag.to_string()))
        .parse_next(input)
}

fn comment(input: &mut Input<'_>) -> ModalResult<Option<Comment>> {
    alt((bracket_comment, semicolon_comment))
        .map(|comment| Text::new(comment).map(Comment))
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
    MoveComment { commands, text: Text::new(comment).map(Comment) }
}

fn command(input: &mut Input<'_>) -> ModalResult<Command> {
    delimited("[%", (name, opt(preceded(space1, parameters))), (space0, ']'))
        .map(|(command, parameters): (Text, Option<Vec<String>>)| Command {
            command,
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

fn merge_comments(into: &mut Option<Comment>, comment: Comment) {
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
        Position,
        formats::san,
        position::{File::*, Role::*, Square::*},
    };

    use super::*;

    fn text(text: &str) -> Text {
        Text::new(text).unwrap()
    }

    fn comment(comment: &str) -> Comment {
        Comment(text(comment))
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

        let game = game.parse(pgn).unwrap();
        assert_eq!(game.tags.len(), 7);
        assert_eq!(game.tags[0], Tag::Event("Casual Game".to_string()));
        assert_eq!(game.moves.len(), 6);
        assert_eq!(game.outcome, Outcome::White);
        assert_eq!(game.moves[5].comment, Some(comment("Italian Game")));
    }

    #[test]
    fn parses_variation_and_nags() {
        let pgn = r#"[Event "x"]

1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *
"#;

        let game = game.parse(pgn).unwrap();
        let e4 = &game.moves[0];
        assert_eq!(
            e4.san.play,
            san::Move::Normal {
                role: Pawn,
                file: None,
                rank: None,
                capture: false,
                to: E4,
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
        let pgn = game
            .parse(
                r#"[Event "x"]

1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *
"#,
            )
            .unwrap();
        let game = crate::Game::try_from(pgn).unwrap();

        assert_eq!(game.roster.event, Some(text("x")));
        assert_eq!(game.tags.len(), 0);
        assert_eq!(game.start_options().len(), 1);

        let e4_id = game.start_options().first().unwrap().slot();
        let e4 = game.play(e4_id).unwrap();
        assert_eq!(e4.play().to, E4);
        assert_eq!(e4.meta.nags, vec![Nag::Symbol("!".to_string())]);
        let e4_options = e4.options();
        assert_eq!(e4_options.len(), 2);

        let (e5, variations) = e4_options.split_first().unwrap();
        assert_eq!(e5.play().to, E5);
        assert_eq!(e5.meta.nags, vec![Nag::Numeric(1)]);

        let c5 = variations.first().unwrap();
        assert_eq!(c5.play().to, C5);
        assert_eq!(c5.meta.comment, Some(text("Sicilian")));
    }

    #[test]
    fn converts_fen_game() {
        let pgn = game
            .parse(
                r#"[FEN "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"]

1. e4 *
"#,
            )
            .unwrap();
        let game = crate::Game::try_from(pgn).unwrap();

        let options = game.start_options();
        let e4 = options.first().unwrap();
        assert_eq!(e4.play().from, E2);
        assert_eq!(e4.play().to, E4);
    }

    #[test]
    fn start_uses_last_fen_tag() {
        let pgn = game
            .parse(
                r#"[FEN "4k3/8/8/8/8/8/8/4K3 w - - 0 1"]
[FEN "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"]

1. e4 *
"#,
            )
            .unwrap();

        assert_eq!(pgn.start.fen(), "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1");
    }

    #[test]
    fn parses_variant_tag() {
        let pgn = r#"
            [Variant "Fischer Random"]
            *
        "#;
        let pgn = game.parse(pgn).unwrap();

        assert_eq!(pgn.tags, vec![Tag::Variant("Fischer Random".to_string())]);
        assert_eq!(pgn.tag_variant().unwrap(), SupportedEnum::Freestyle);
        assert!(pgn.freestyle().unwrap());
        assert!(pgn.to_string().contains("[Variant \"Fischer Random\"]"));
    }

    #[test]
    fn variant_folds_over_tags() {
        let pgn = r#"
            [Variant "Chess960"]
            [Variant "Standard"]
            *
        "#;
        let pgn = game.parse(pgn).unwrap();

        assert_eq!(pgn.tag_variant().unwrap(), SupportedEnum::Chess);
        assert!(!pgn.freestyle().unwrap());
    }

    #[test]
    fn variant_rejects_unsupported_tags() {
        let pgn = r#"
            [Variant "Antichess"]
            *
        "#;
        let pgn = game.parse(pgn).unwrap();

        assert_eq!(pgn.tag_variant().unwrap_err(), "Antichess");
        assert_eq!(pgn.freestyle().unwrap_err(), "Antichess");
    }

    #[test]
    fn freestyle_falls_back_to_position_castling() {
        let fen = r#"
            [FEN "8/8/8/8/8/8/8/8 w - - 0 1"]
            *
        "#;
        let pgn = game.parse(fen).unwrap();

        assert!(!pgn.freestyle().unwrap());

        let fen = r#"
            [FEN "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9"]
            *
        "#;
        let pgn = game.parse(fen).unwrap();

        assert!(pgn.freestyle().unwrap());
    }

    #[test]
    fn displays_fen_game_from_its_start_ply() {
        let fen = "4k3/8/8/8/8/8/4P3/4K3 b - - 0 17";
        let position = fen::parse_position.parse(fen).unwrap().validate().unwrap();
        let mut game = crate::Game::new(position);
        game.start_options_mut().push(crate::Move::normal(King, E8, D8)).unwrap();

        let pgn = Game::from(game);
        assert!(pgn.to_string().contains("[SetUp \"1\"]"), "{}", pgn);
        assert!(pgn.to_string().contains(&format!("[FEN \"{fen}\"]")), "{}", pgn);
        assert!(pgn.to_string().contains("\n17... Kd8 *"), "{}", pgn);
        assert_eq!(pgn.movetext(), "17... Kd8");
    }

    #[test]
    fn displays_figurine_movetext() {
        let mut game = crate::Game::new(Position::start());
        let e4 = game.start_options_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap();
        game.play_mut(e4).unwrap().options_mut().push(crate::Move::normal(Knight, G8, F6)).unwrap();

        let pgn = Game::from(game);
        assert_eq!(pgn.movetext(), "1. e4 Nf6");
        assert_eq!(pgn.figurine_movetext(), "1. e4 ♘f6");
    }

    #[test]
    fn converts_from_game() {
        let mut game = crate::Game::new(Position::start());
        game.roster.event = Some(text("x"));

        let e4 = game.start_options_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap();
        game.start_options_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap();

        {
            let mut e4 = game.play_mut(e4).unwrap();
            e4.meta.nags.push(Nag::Symbol("!".to_string()));
            e4.options_mut().push(crate::Move::normal(Pawn, E7, E5)).unwrap();
            let c5 = e4.options_mut().push(crate::Move::normal(Pawn, C7, C5)).unwrap();
            game.play_mut(c5).unwrap().meta.comment = Some(text("Sicilian"));
        }

        let pgn = Game::from(game);
        assert_eq!(
            pgn.to_string(),
            r#"[Event "x"]
[Result "*"]

1. e4! (1. d4) 1... e5 (1... c5 {Sicilian}) *"#
        );
    }

    #[test]
    fn roundtrips_kiwipete_game_tree() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let position = fen::parse_position.parse(fen).unwrap().validate().unwrap();
        let mut game = crate::Game::new(position);

        for play in position.legal_moves() {
            let id = game.start_options_mut().push(play).unwrap();
            let replies = game.play(id).unwrap().legal().to_vec();
            let mut play = game.play_mut(id).unwrap();
            for reply in replies {
                play.options_mut().push(reply).unwrap();
            }
        }

        let pgn = Game::from(game);
        let roundtrip = Game::from(crate::Game::try_from(pgn.clone()).unwrap());

        assert_eq!(roundtrip, pgn);
    }

    #[test]
    fn parses_annotations_between_variations() {
        let pgn = r#"[Event "x"]

1. e4 e5 {A} (1... c5) {B} (1... e6) {C} 2. Nf3 *
"#;

        let game = game.parse(pgn).unwrap();
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
        let game =
            game.parse(r#"[Event "x"] 1. e4 {[%cal Ge2e4] [%clk 0:14:49] Good move.} *"#).unwrap();
        let e4 = &game.moves[0];
        assert_eq!(
            e4.annotations,
            vec![
                Annotation::Command(Command {
                    command: text("cal"),
                    parameters: vec!["Ge2e4".to_string()],
                }),
                Annotation::Command(Command {
                    command: text("clk"),
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
        let game = game
            .parse(r#"[Event "x"] 1. e4 {[%tqu "En","find the move","","","e2e4","",10]} *"#)
            .unwrap();
        let e4 = &game.moves[0];
        assert_eq!(
            e4.annotations,
            vec![Annotation::Command(Command {
                command: text("tqu"),
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
        let game = game.parse(r#"[Event "x"] 1. e4 {[%foo ]} *"#).unwrap();
        assert_eq!(
            game.moves[0].annotations,
            vec![Annotation::Command(Command { command: text("foo"), parameters: vec![] })]
        );
        assert_eq!(
            game.to_string(),
            r#"[Event "x"]

1. e4 {[%foo]} *"#
        );
    }

    #[test]
    fn ignores_empty_comments() {
        let game = game.parse(r#"[Event "x"] 1. e4 {} {   } e5 *"#).unwrap();
        assert_eq!(game.moves[0].comment, None);
        assert_eq!(game.moves[1].comment, None);
    }

    #[test]
    fn parses_san_in_moves() {
        let game = game.parse(r#"[Event "x"] 1. exd8=Q# *"#).unwrap();
        assert_eq!(
            game.moves[0].san,
            san::San {
                play: san::Move::Normal {
                    role: Pawn,
                    file: Some(E),
                    rank: None,
                    capture: true,
                    to: D8,
                    promotion: Some(Queen),
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
            "[Event \"x\"]\n\n1. e4! 1... e5 $1 (1... c5 {Sicilian}) 2. Nf3 *"
        );
    }

    #[test]
    fn reads_games_one_at_a_time() {
        let input = b"[Event \"a\"]\n1. e4 *\n\n[Event \"b\"]\n1. d4 *\n";
        let games =
            stream::games(&input[..]).map(|game| game.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].tags[0], Tag::Event("a".to_string()));
        assert_eq!(games[1].tags[0], Tag::Event("b".to_string()));
    }

    #[test]
    fn reader_does_not_split_inside_comment() {
        let input = b"[Event \"a\"]\n1. e4 {\n[not a tag]\n} *\n\n[Event \"b\"]\n1. d4 *\n";
        let games =
            stream::games(&input[..]).map(|game| game.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].moves[0].comment, Some(comment("[not a tag]")));
    }

    #[test]
    fn reader_handles_movetext_after_tag_on_same_line() {
        let input = b"[Event \"a\"] 1. e4 *\n\n[Event \"b\"] 1. d4 *\n";
        let games =
            stream::games(&input[..]).map(|game| game.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].tags[0], Tag::Event("a".to_string()));
        assert_eq!(games[1].tags[0], Tag::Event("b".to_string()));
    }

    #[test]
    fn parses_line_wrapped_black_move_without_number() {
        game.parse("[Event \"x\"]\n\n14. f3 b6\n15. Be2 *").unwrap();
    }
}
