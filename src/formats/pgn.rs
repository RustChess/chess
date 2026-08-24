//! PGN format

use std::{collections::BTreeMap, fmt};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub tag_pairs: Vec<TagPair>,
    pub variation: Variation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Games {
    pub games: Vec<Game>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variation {
    pub intro: Vec<Annotation>,
    pub moves: Vec<Move>,
    pub outcome: Option<Outcome>,
    pub outro: Vec<Annotation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Move {
    pub san: san::San,
    pub intro: Vec<Annotation>,
    pub variations: Variations,
    pub outro: Vec<Annotation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Variations {
    pub variations: Vec<Variation>,
    /// Annotations before `variations[index]`.
    ///
    /// Keys are in `1..variations.len()`: annotations before the first variation
    /// belong to `Move::intro`, and annotations after the last variation belong
    /// to `Move::outro`.
    pub before: BTreeMap<usize, Vec<Annotation>>,
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
        self.variation.write(&mut wrap, 0)
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

impl Variation {
    fn write(&self, wrap: &mut Wrap<'_, '_>, first_ply: usize) -> fmt::Result {
        let mut ply = first_ply;
        for annotation in &self.intro {
            wrap.token(annotation)?;
        }
        for (index, play) in self.moves.iter().enumerate() {
            if should_write_move_number(ply, index, &self.moves) {
                wrap.token(format!("{} {}", MoveNumber(ply), play.san))?;
            } else {
                wrap.token(play.san)?;
            }
            play.write_tail(wrap, ply)?;
            ply += 1;
        }
        if let Some(outcome) = self.outcome {
            wrap.token(outcome)?;
        }
        for annotation in &self.outro {
            wrap.token(annotation)?;
        }
        Ok(())
    }
}

impl Move {
    fn write_tail(&self, wrap: &mut Wrap<'_, '_>, ply: usize) -> fmt::Result {
        for annotation in &self.intro {
            wrap.token(annotation)?;
        }
        self.variations.write(wrap, ply)?;
        for annotation in &self.outro {
            wrap.token(annotation)?;
        }
        Ok(())
    }
}

impl Variations {
    fn write(&self, wrap: &mut Wrap<'_, '_>, ply: usize) -> fmt::Result {
        for (index, variation) in self.variations.iter().enumerate() {
            if let Some(annotations) = self.before.get(&index) {
                for annotation in annotations {
                    wrap.token(annotation)?;
                }
            }
            wrap.open("(")?;
            variation.write(wrap, ply)?;
            wrap.close(")")?;
        }
        Ok(())
    }
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
        !self.intro.is_empty() || !self.variations.variations.is_empty() || !self.outro.is_empty()
    }
}

fn escape_tag_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn game(input: &mut Input<'_>) -> ModalResult<Game> {
    (tag_pairs, root_variation.context(StrContext::Label("PGN movetext")))
        .map(|(tag_pairs, variation)| Game { tag_pairs, variation })
        .context(StrContext::Label("PGN game"))
        .parse_next(input)
}

pub fn games(input: &mut Input<'_>) -> ModalResult<Games> {
    let mut parsed = Vec::new();
    loop {
        trim(input);
        if input.is_empty() {
            break;
        }
        parsed.push(game(input)?);
    }
    Ok(Games { games: parsed })
}

pub fn tag_pairs(input: &mut Input<'_>) -> ModalResult<Vec<TagPair>> {
    let mut tags = Vec::new();
    loop {
        trim(input);
        if !input.starts_with('[') {
            break;
        }
        tags.push(tag_pair(input)?);
    }
    Ok(tags)
}

pub fn tag_pair(input: &mut Input<'_>) -> ModalResult<TagPair> {
    ('[', tag_name, space1, tag_value, space0, ']')
        .map(|(_, name, _, value, _, _)| TagPair { name, value })
        .context(StrContext::Label("PGN tag pair"))
        .parse_next(input)
}

fn root_variation(input: &mut Input<'_>) -> ModalResult<Variation> {
    let variation = variation_body(input, false)?;
    trim(input);
    Ok(variation)
}

fn variation(input: &mut Input<'_>) -> ModalResult<Variation> {
    ('(', |input: &mut Input<'_>| variation_body(input, true), ')')
        .map(|(_, variation, _)| variation)
        .context(StrContext::Label("PGN variation"))
        .parse_next(input)
}

fn variation_body(input: &mut Input<'_>, nested: bool) -> ModalResult<Variation> {
    let intro = annotations(input)?;
    let mut moves = Vec::new();
    let mut result = None;

    loop {
        skip_move_numbers(input)?;
        trim(input);

        if input.is_empty() || input.starts_with('[') || (nested && input.starts_with(')')) {
            break;
        }

        if let Some(done) = outcome(input)? {
            result = Some(done);
            break;
        }

        moves.push(parse_move(input)?);
    }

    let outro = annotations(input)?;
    Ok(Variation { intro, moves, outcome: result, outro })
}

fn parse_move(input: &mut Input<'_>) -> ModalResult<Move> {
    let san = san::san.context(StrContext::Label("PGN move")).parse_next(input)?;
    let intro = annotations(input)?;
    let (variations, outro) = variations(input)?;
    Ok(Move { san, intro, variations, outro })
}

fn variations(input: &mut Input<'_>) -> ModalResult<(Variations, Vec<Annotation>)> {
    let mut variations = Vec::new();
    let mut before = BTreeMap::new();
    let mut pending = Vec::new();

    loop {
        skip_move_numbers(input)?;
        trim(input);

        if !input.starts_with('(') {
            break;
        }

        let index = variations.len();
        if index > 0 && !pending.is_empty() {
            before.insert(index, pending);
        }

        variations.push(variation(input)?);
        pending = annotations(input)?;
    }

    Ok((Variations { variations, before }, pending))
}

fn annotations(input: &mut Input<'_>) -> ModalResult<Vec<Annotation>> {
    let mut annotations = Vec::new();
    loop {
        trim(input);
        if input.starts_with('{') {
            annotations.push(Annotation::Comment(comment(input)?));
        } else if input.starts_with(';') {
            annotations.push(Annotation::Comment(line_comment(input)?));
        } else if input.starts_with('$') {
            annotations.push(Annotation::Nag(numeric_nag(input)?));
        } else if input.starts_with('!') || input.starts_with('?') {
            annotations.push(Annotation::Nag(symbol_nag(input)?));
        } else {
            break;
        }
    }
    Ok(annotations)
}

fn skip_move_numbers(input: &mut Input<'_>) -> ModalResult<()> {
    loop {
        trim(input);
        let snapshot = *input;

        if input.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            let _number = dec_uint::<_, u32, _>.parse_next(input)?;
            let dots = take_while(input, |c| c == '.')?;
            if dots.len() == 1 || dots.len() == 3 {
                continue;
            }
        }

        *input = snapshot;
        break;
    }
    Ok(())
}

fn tag_name(input: &mut Input<'_>) -> ModalResult<Tag> {
    let name = take_while(input, |c| c.is_ascii_alphanumeric() || c == '_')?;
    if name.is_empty() {
        return err();
    }
    Ok(match name {
        "Event" => Tag::Event,
        "Site" => Tag::Site,
        "Date" => Tag::Date,
        "Round" => Tag::Round,
        "White" => Tag::White,
        "Black" => Tag::Black,
        "Result" => Tag::Result,
        _ => Tag::Other(name.to_string()),
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

fn numeric_nag(input: &mut Input<'_>) -> ModalResult<Nag> {
    preceded('$', dec_uint).map(Nag::Numeric).parse_next(input)
}

fn symbol_nag(input: &mut Input<'_>) -> ModalResult<Nag> {
    alt(("!!", "!?", "?!", "??", "!", "?"))
        .map(|nag: &str| Nag::Symbol(nag.to_string()))
        .parse_next(input)
}

fn comment(input: &mut Input<'_>) -> ModalResult<String> {
    preceded('{', terminated(take_till(0.., '}'), '}'))
        .map(|comment: &str| comment.trim().to_string())
        .context(StrContext::Label("PGN comment"))
        .parse_next(input)
}

fn line_comment(input: &mut Input<'_>) -> ModalResult<String> {
    preceded(';', take_till(0.., '\n'))
        .map(|comment: &str| comment.trim().to_string())
        .parse_next(input)
}

fn outcome(input: &mut Input<'_>) -> ModalResult<Option<Outcome>> {
    trim(input);
    Ok([
        ("1/2-1/2", Outcome::Draw),
        ("1-0", Outcome::White),
        ("0-1", Outcome::Black),
        ("*", Outcome::Unknown),
    ]
    .into_iter()
    .find_map(|(token, outcome)| {
        token_boundary(input, token).then(|| {
            advance(input, token.len());
            outcome
        })
    }))
}

fn token_boundary(input: &str, token: &str) -> bool {
    input.starts_with(token)
        && input[token.len()..].chars().next().is_none_or(|c| {
            c.is_whitespace() || matches!(c, '[' | ']' | '{' | '}' | '(' | ')' | ';')
        })
}

fn trim(input: &mut Input<'_>) {
    *input = input.trim_start();
}

fn take_while<'a>(
    input: &mut &'a str,
    mut predicate: impl FnMut(char) -> bool,
) -> ModalResult<&'a str> {
    let mut end = 0;
    for (index, c) in input.char_indices() {
        if !predicate(c) {
            break;
        }
        end = index + c.len_utf8();
    }
    let (taken, rest) = input.split_at(end);
    *input = rest;
    Ok(taken)
}

fn advance(input: &mut Input<'_>, bytes: usize) {
    *input = &input[bytes..];
}

fn err<T>() -> ModalResult<T> {
    Err(ErrMode::Backtrack(ContextError::new()))
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
        assert_eq!(game.variation.moves.len(), 6);
        assert_eq!(game.variation.outcome, Some(Outcome::White));
        assert_eq!(
            game.variation.moves[5].intro,
            vec![Annotation::Comment("Italian Game".to_string())]
        );
    }

    #[test]
    fn parses_variation_and_nags() {
        let pgn = r#"[Event "x"]

1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *
"#;

        let game = game.parse(pgn).unwrap();
        let e4 = &game.variation.moves[0];
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
        assert_eq!(e4.intro, vec![Annotation::Nag(Nag::Symbol("!".to_string()))]);

        let e5 = &game.variation.moves[1];
        assert_eq!(e5.intro, vec![Annotation::Nag(Nag::Numeric(1))]);
        assert_eq!(e5.variations.variations.len(), 1);
        assert_eq!(
            e5.variations.variations[0].moves[0].intro,
            vec![Annotation::Comment("Sicilian".to_string())]
        );
        assert_eq!(game.variation.outcome, Some(Outcome::Unknown));
    }

    #[test]
    fn parses_annotations_between_variations() {
        let pgn = r#"[Event "x"]

1. e4 e5 {A} (1... c5) {B} (1... e6) {C} 2. Nf3 *
"#;

        let game = game.parse(pgn).unwrap();
        let e5 = &game.variation.moves[1];
        assert_eq!(e5.intro, vec![Annotation::Comment("A".to_string())]);
        assert_eq!(e5.variations.variations.len(), 2);
        assert_eq!(e5.variations.before.get(&1), Some(&vec![Annotation::Comment("B".to_string())]));
        assert_eq!(e5.outro, vec![Annotation::Comment("C".to_string())]);
    }

    #[test]
    fn parses_san_in_moves() {
        let game = game.parse(r#"[Event "x"] 1. exd8=Q# *"#).unwrap();
        assert_eq!(
            game.variation.moves[0].san,
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
}
