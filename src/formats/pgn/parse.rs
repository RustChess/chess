use crate::game::{Command, Nag, Outcome, Tag as OtherTag, Text};

use super::*;
use crate::formats::{StrInput as Input, fen, prelude::*, san};

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

impl Error {
    pub fn from(
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

pub fn strip_tags(mut input: &str) -> &str {
    loop {
        let before = input;
        if tag.parse_next(&mut input).is_err() {
            return before;
        }
    }
}

pub fn tag_start_ok(mut input: &str) -> bool {
    tag.parse_next(&mut input).is_ok()
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

pub fn parse_move(input: &mut Input<'_>) -> ModalResult<Move> {
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

pub fn variations(input: &mut Input<'_>) -> ModalResult<Vec<Variation>> {
    repeat(0.., preceded(multispace0, variation)).parse_next(input)
}

// ({intro} 1... c5 {move comment}) {outro}
pub fn variation(input: &mut Input<'_>) -> ModalResult<Variation> {
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

pub fn comment(input: &mut Input<'_>) -> ModalResult<Option<Comment>> {
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
