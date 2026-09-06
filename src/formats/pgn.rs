//! PGN format

use std::{fmt, str};

use winnow::Parser as _;

use crate::{
    Position, Scharnagl,
    game::{Command, Mode, Nag, Outcome, Slot, Tag as OtherTag, Text},
    position::Parts,
};

use super::san;

pub mod convert;
pub mod parse;
pub mod stream;

pub use parse::game;

use Mode::*;

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
#[cfg_attr(feature = "serde", derive(DeserializeFromStr, SerializeDisplay))]
pub struct Game {
    pub tags: Vec<Tag>,
    pub start: Parts,
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
    Fen(Parts),
    SetUp(bool),
    Variant(String),
    Chess960Id(Scharnagl),
    Other(OtherTag),
}

impl Tag {
    pub fn freestyle() -> Self {
        Tag::Variant("Fischerandom".to_string())
    }

    pub fn variant(&self) -> Option<&str> {
        if let Tag::Variant(value) = self { Some(value) } else { None }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Parse(#[from] parse::Error),
    #[error(transparent)]
    Convert(#[from] convert::Error),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl fmt::Display for Game {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shredder = self.mode().is_freestyle();
        for tag in &self.tags {
            if let Tag::Fen(position) = tag {
                let fen = if shredder { position.shredder_fen() } else { position.fen() };
                write_tag(f, "FEN", &fen)?;
                writeln!(f)?;
            } else {
                writeln!(f, "{tag}")?;
            }
        }
        if !self.tags.is_empty() {
            writeln!(f)?;
        }
        let mut wrap = Wrap::<_, 80>::new(f);
        if let Some(intro) = &self.intro {
            wrap.token(intro)?;
        }
        write_moves(&self.moves, &mut wrap, self.start.ply(), Notation::San)?;
        wrap.token(self.outcome)
    }
}

impl str::FromStr for Game {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        parse::game.parse(text).map_err(|error| parse::Error::from(text, 1, error).into())
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
        write_moves(&self.moves, &mut wrap, self.start.ply(), notation)
            .expect("writing PGN movetext to string");
        movetext
    }

    pub fn mode(&self) -> Mode {
        for tag in &self.tags {
            let Some(variant) = tag.variant() else { continue };
            if Mode::from_tag(variant) == Some(Freestyle) {
                return Freestyle;
            }
        }

        if self.start.castles.chess_compatible() { Chess } else { Freestyle }
    }
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
            Tag::Chess960Id(id) => write_tag(f, "Chess960Id", &id.to_string()),
            Tag::Other(tag) => write_tag(f, tag.key.as_ref(), &tag.value),
        }
    }
}

impl Mode {
    fn from_tag(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "chess" | "standard" => Some(Chess),
            "chess960" | "fischerandom" | "fischer random" | "freestyle" => Some(Freestyle),
            _ => None,
        }
    }
}

fn write_tag(f: &mut fmt::Formatter<'_>, key: &str, value: &str) -> fmt::Result {
    fn escape_tag_value(value: &str) -> String {
        value.replace('\\', "\\\\").replace('"', "\\\"")
    }

    write!(f, "[{} \"{}\"]", key, escape_tag_value(value))
}

fn start_position(tags: &[Tag]) -> Parts {
    tags.iter()
        .rev()
        .find_map(|tag| match tag {
            Tag::Fen(position) => Some(*position),
            _ => None,
        })
        .unwrap_or_else(|| Position::start().parts())
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

#[cfg(test)]
mod tests {
    use crate::{
        Position,
        board::{Role::*, Scharnagl},
        formats::san,
        square::{File::*, Square::*},
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
        let game = crate::Game::from_pgn(pgn).unwrap();

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
        let game = crate::Game::from_pgn(pgn).unwrap();

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
        assert_eq!(pgn.mode(), Freestyle);
        assert!(pgn.to_string().contains("[Variant \"Fischer Random\"]"));
    }

    #[test]
    fn freestyle_variant_wins() {
        let pgn = r#"
            [Variant "Chess960"]
            [Variant "Standard"]
            *
        "#;
        let pgn = game.parse(pgn).unwrap();

        assert_eq!(pgn.mode(), Freestyle);
    }

    #[test]
    fn freestyle_variant_overrides_unsupported() {
        let pgn = r#"
            [Variant "Antichess"]
            [Variant "Chess960"]
            *
        "#;
        let pgn = game.parse(pgn).unwrap();

        assert_eq!(pgn.mode(), Freestyle);

        let pgn = r#"
            [Variant "Chess960"]
            [Variant "Antichess"]
            *
        "#;
        let pgn = game.parse(pgn).unwrap();

        assert_eq!(pgn.mode(), Freestyle);
    }

    #[test]
    fn mode_ignores_unsupported_tags() {
        let pgn = r#"
            [Variant "Antichess"]
            *
        "#;
        let pgn = game.parse(pgn).unwrap();

        assert_eq!(pgn.mode(), Chess);
    }

    #[test]
    fn mode_falls_back_to_position_castling() {
        let fen = r#"
            [FEN "8/8/8/8/8/8/8/8 w - - 0 1"]
            *
        "#;
        let pgn = game.parse(fen).unwrap();

        assert_eq!(pgn.mode(), Chess);

        let fen = r#"
            [Variant "Standard"]
            [FEN "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9"]
            *
        "#;
        let pgn = game.parse(fen).unwrap();

        assert_eq!(pgn.mode(), Freestyle);
        assert!(pgn.to_string().contains(" w HFhf - 2 9\"]"));
    }

    #[test]
    fn converts_to_game_mode() {
        let pgn = game.parse(r#"[Event "x"] 1. e4 *"#).unwrap();
        let chess = crate::Game::try_from(pgn).unwrap();
        assert_eq!(chess.mode(), Chess);

        let pgn = game
            .parse(
                r#"[Variant "Chess960"]
[FEN "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9"]
9. e4 *"#,
            )
            .unwrap();
        let freestyle = crate::Game::try_from(pgn).unwrap();
        assert_eq!(freestyle.mode(), Freestyle);
    }

    #[test]
    fn converts_unsupported_variant_as_chess() {
        let pgn = game.parse(r#"[Variant "Antichess"] *"#).unwrap();
        let game = crate::Game::try_from(pgn).unwrap();

        assert_eq!(game.mode(), Chess);
    }

    #[test]
    fn rejects_invalid_start() {
        let pgn =
            game.parse(r#"[Variant "Standard"] [FEN "4k3/8/8/8/8/8/8/4K2P w - - 0 1"] *"#).unwrap();
        let error = crate::Game::try_from(pgn).err().unwrap();

        assert!(matches!(error, convert::Error::Start { mode: Chess, .. }));
    }

    #[test]
    fn displays_fen_game_from_its_start_ply() {
        let fen = "4k3/8/8/8/8/8/4P3/4K3 b - - 0 17";
        let position = Position::from_fen(fen).unwrap();
        let mut game = crate::Game::chess(position).unwrap();
        game.start_options_mut().push(crate::Move::normal(King, E8, D8)).unwrap();

        let pgn = Game::from(game);
        assert!(pgn.to_string().contains("[SetUp \"1\"]"), "{}", pgn);
        assert!(pgn.to_string().contains(&format!("[FEN \"{fen}\"]")), "{}", pgn);
        assert!(pgn.to_string().contains("\n17... Kd8 *"), "{}", pgn);
        assert_eq!(pgn.movetext(), "17... Kd8");
    }

    #[test]
    fn displays_figurine_movetext() {
        let mut game = crate::Game::chess(Position::start()).unwrap();
        let e4 = game.start_options_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().slot();
        game.play_mut(e4).unwrap().options_mut().push(crate::Move::normal(Knight, G8, F6)).unwrap();

        let pgn = Game::from(game);
        assert_eq!(pgn.movetext(), "1. e4 Nf6");
        assert_eq!(pgn.figurine_movetext(), "1. e4 ♘f6");
    }

    #[test]
    fn converts_from_game() {
        let mut game = crate::Game::chess(Position::start()).unwrap();
        game.roster.event = Some(text("x"));

        let e4 = game.start_options_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().slot();
        game.start_options_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap();

        {
            let mut e4 = game.play_mut(e4).unwrap();
            e4.meta.nags.push(Nag::Symbol("!".to_string()));
            e4.options_mut().push(crate::Move::normal(Pawn, E7, E5)).unwrap();
            let c5 = e4.options_mut().push(crate::Move::normal(Pawn, C7, C5)).unwrap().slot();
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
    fn converts_from_freestyle_game() {
        let game = crate::Game::freestyle(Position::freestyle(Scharnagl::CHESS));

        let pgn = Game::from(game);

        assert!(pgn.to_string().contains("[Variant \"Fischerandom\"]"), "{}", pgn);
        assert!(pgn.to_string().contains("[SetUp \"1\"]"), "{}", pgn);
        assert!(pgn.to_string().contains("[Chess960Id \"518\"]"), "{}", pgn);
        assert!(
            pgn.to_string()
                .contains("[FEN \"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w HAha - 0 1\"]"),
            "{}",
            pgn
        );
    }

    #[test]
    fn roundtrips_kiwipete_game_tree() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let position = Position::from_fen(fen).unwrap();
        let mut game = crate::Game::chess(position).unwrap();

        for play in position.legal_moves() {
            let id = game.start_options_mut().push(play).unwrap().slot();
            let replies = game.play(id).unwrap().legal().to_vec();
            let mut play = game.play_mut(id).unwrap();
            for reply in replies {
                play.options_mut().push(reply).unwrap();
            }
        }

        let pgn = Game::from(game);
        let game = crate::Game::from_pgn(pgn.clone()).unwrap();
        let roundtrip = Game::from(game);

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
