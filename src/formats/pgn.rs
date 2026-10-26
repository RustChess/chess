//! PGN format

use core::{fmt, ops, str};

use winnow::Parser as _;

use crate::{
    Player, Position, Scharnagl,
    game::{self, Mode, Nag, Outcome},
    position::{self, Parts},
};

use super::{Text, san};

pub mod command;
pub mod convert;
pub mod datetime;
pub mod parse;
pub mod stream;
pub mod tag;
pub mod time_control;

pub use parse::pgn;
pub use tag::Eco;

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

#[derive(Clone, Debug, DeserializeFromStr, Eq, Ord, PartialEq, PartialOrd, SerializeDisplay)]
pub struct Pgn {
    pub tags: Tags,
    pub start: Parts,
    pub intro: Option<Comment>,
    pub moves: Vec<Move>,
    pub outcome: Option<Outcome>,
    pub outro: Option<Comment>,
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Tags(Vec<Tag>);

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Commands(Vec<Command>);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Command {
    Clock(command::Clock),
    ColoredArrows(command::ColoredArrows),
    ColoredSquares(command::ColoredSquares),
    ElapsedGame(command::ElapsedGame),
    ElapsedMove(command::ElapsedMove),
    Evaluation(Eval),
    Expanded,
    MechanicalClock(command::MechanicalClock),
    Other(command::Other),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Tag {
    Title(String),
    Annotator(Text),
    Event(String),
    EventDate(Date),
    EventType(Text),
    EventCategory(Text),
    EventRounds(Text),
    EventCountry(Text),
    Id(Text),
    Site(String),
    Date(Date),
    EndDate(Date),
    EndTime(Time),
    Time(Time),
    TimeControl(TimeControl),
    Timezone(Text),
    UtcDate(Date),
    UtcTime(Time),
    Round(String),
    White(String),
    WhiteName(Text),
    WhiteTitle(Text),
    WhiteElo(u16),
    WhiteFideId(u32),
    WhiteTeam(Text),
    WhiteTeamCountry(Text),
    Black(String),
    BlackName(Text),
    BlackTitle(Text),
    BlackElo(u16),
    BlackFideId(u32),
    BlackTeam(Text),
    BlackTeamCountry(Text),
    Outcome(Option<Outcome>),
    Rated(bool),
    Termination(Text),
    Eco(Eco),
    Fen(Parts),
    SetUp(bool),
    Variant(String),
    Chess960Id(Scharnagl),
    Orientation(Player),
    TimeType(Text),
    Source(Text),
    SourceDate(Date),
    Publisher(Text),
    Valuation(Eval),
    Other(tag::Other),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Date {
    year: [Option<u8>; 4],
    month: [Option<u8>; 2],
    day: [Option<u8>; 2],
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Time {
    hour: [Option<u8>; 2],
    minute: [Option<u8>; 2],
    second: [Option<u8>; 2],
}

#[derive(Clone, Debug, DeserializeFromStr, Eq, Ord, PartialEq, PartialOrd, SerializeDisplay)]
pub enum TimeControl {
    Unknown,
    None,
    Periods(Vec<Period>),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Period {
    Moves { moves: u32, seconds: u32 },
    SuddenDeath { seconds: u32 },
    Increment { seconds: u32, increment: u32 },
    Hourglass { seconds: u32 },
}

/// PGN import settings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Import {
    pub eval: import::Eval,
}

pub mod import {
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub enum Eval {
        Drop,
        #[default]
        Passthrough,
    }
}

/// PGN export settings.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Export {
    pub eval: export::Eval,
}

pub mod export {
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub enum Eval {
        #[default]
        Passthrough,
        Overwrite,
    }
}

impl Tags {
    pub fn from_pgn(text: &str) -> Result<Self> {
        parse::tag_section
            .parse(text)
            .map(Into::into)
            .map_err(|error| parse::Error::from(text, 1, error).into())
    }

    pub fn find(&self, name: &str) -> Option<&Tag> {
        self.iter().find(|tag| tag.name() == name)
    }

    pub fn rfind(&self, name: &str) -> Option<&Tag> {
        self.iter().rev().find(|tag| tag.name() == name)
    }

    pub fn date(&self) -> Option<Date> {
        match self.rfind("Date")? {
            Tag::Date(date) => Some(*date),
            _ => None,
        }
    }

    pub fn time(&self) -> Option<Time> {
        match self.rfind("Time")? {
            Tag::Time(time) => Some(*time),
            _ => None,
        }
    }

    pub fn time_control(&self) -> Option<&TimeControl> {
        match self.rfind("TimeControl")? {
            Tag::TimeControl(time_control) => Some(time_control),
            _ => None,
        }
    }

    pub fn utc_date(&self) -> Option<Date> {
        match self.rfind("UTCDate")? {
            Tag::UtcDate(date) => Some(*date),
            _ => None,
        }
    }

    pub fn utc_time(&self) -> Option<Time> {
        match self.rfind("UTCTime")? {
            Tag::UtcTime(time) => Some(*time),
            _ => None,
        }
    }

    pub fn utc_timestamp(&self) -> Option<::time::UtcDateTime> {
        Some(self.utc_date()?.date()?.with_time(self.utc_time()?.time()?).as_utc())
    }
}

impl From<Vec<Tag>> for Tags {
    fn from(tags: Vec<Tag>) -> Self {
        Self(tags)
    }
}

impl ops::Deref for Tags {
    type Target = Vec<Tag>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for Tags {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Commands {
    pub fn colored_arrows(&self) -> impl Iterator<Item = &game::Arrow> {
        self.iter().flat_map(|command| match command {
            Command::ColoredArrows(arrows) => arrows.iter(),
            _ => [].iter(),
        })
    }

    pub fn colored_squares(&self) -> impl Iterator<Item = &game::Marker> {
        self.iter().flat_map(|command| match command {
            Command::ColoredSquares(markers) => markers.iter(),
            _ => [].iter(),
        })
    }
}

impl From<Vec<Command>> for Commands {
    fn from(commands: Vec<Command>) -> Self {
        Self(commands)
    }
}

impl ops::Deref for Commands {
    type Target = Vec<Command>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for Commands {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl IntoIterator for Commands {
    type Item = Command;
    type IntoIter = std::vec::IntoIter<Command>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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

impl fmt::Display for Pgn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shredder = self.mode().is_freestyle();
        for tag in self.tags.iter() {
            if let Tag::Fen(position) = tag {
                let fen = if shredder { position.shredder_fen() } else { position.fen() };
                tag::write_tag(f, "FEN", &fen)?;
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
        write_plays(&self.moves, &mut wrap, self.start.ply(), Notation::San)?;
        wrap.token(self.outcome.map(Outcome::result).unwrap_or("*"))?;
        if let Some(outro) = &self.outro {
            wrap.token(outro)?;
        }
        Ok(())
    }
}

impl str::FromStr for Pgn {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        parse::pgn.parse(text).map_err(|error| parse::Error::from(text, 1, error).into())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Variation {
    pub intro: Option<Comment>,
    pub moves: Vec<Move>,
    pub outro: Option<Comment>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Move {
    pub san: san::San,
    pub comment: Option<Comment>,
    pub annotations: Vec<Annotation>,
    pub variations: Vec<Variation>,
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Comment {
    pub commands: Commands,
    pub text: Option<Text>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Score {
    Centipawns(i32),
    Mate(i32),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Eval {
    pub depth: Option<u16>,
    pub score: Score,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Annotation {
    Nag(Nag),
    Valuation(Eval),
    Command(Command),
}

impl fmt::Display for Eval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.score {
            Score::Centipawns(centipawns) => {
                write!(f, "{:.2}", f64::from(centipawns) / 100.0)?;
            }
            Score::Mate(mate) => write!(f, "#{mate}")?,
        }
        if let Some(depth) = self.depth {
            write!(f, ",{depth}")?;
        }
        Ok(())
    }
}

impl Eval {
    fn command(self) -> Command {
        Command::Evaluation(self)
    }
}

impl str::FromStr for Eval {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (score, depth) = match value.trim().split_once(',') {
            Some((score, depth)) => {
                let depth = depth.trim().parse().map_err(|_| "invalid evaluation depth")?;
                (score, Some(depth))
            }
            None => (value, None),
        };
        let score = score.trim();
        let score = if let Some(mate) = score.strip_prefix('#') {
            Score::Mate(mate.parse().map_err(|_| "invalid mate score")?)
        } else {
            let pawns: f64 = score.parse().map_err(|_| "invalid centipawn score")?;
            let centipawns = (pawns * 100.0).round();
            if !centipawns.is_finite()
                || centipawns < f64::from(i32::MIN)
                || centipawns > f64::from(i32::MAX)
            {
                return Err("centipawn score out of range");
            }
            Score::Centipawns(centipawns as i32)
        };
        Ok(Self { score, depth })
    }
}

impl From<position::Score> for Score {
    fn from(score: position::Score) -> Self {
        use position::Score::*;

        match score {
            Centipawns(value) => Score::Centipawns(value),
            Mate(value) => Score::Mate(value.get()),
        }
    }
}

impl From<&position::Analysis> for Option<Eval> {
    fn from(analysis: &position::Analysis) -> Self {
        let position::Analysis::Evaluation(evaluation) = analysis else { return None };
        Some(Eval { depth: Some(evaluation.depth), score: evaluation.score().into() })
    }
}

impl Pgn {
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
        write_plays(&self.moves, &mut wrap, self.start.ply(), notation)
            .expect("writing PGN movetext to string");
        movetext
    }

    pub fn mode(&self) -> Mode {
        for tag in self.tags.iter() {
            let Some(variant) = tag.variant() else { continue };
            if Mode::from_tag(variant) == Some(Freestyle) {
                return Freestyle;
            }
        }

        if self.start.castles.chess_compatible() { Chess } else { Freestyle }
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
            Annotation::Valuation(valuation) => valuation.command().fmt(f),
            Annotation::Command(command) => write!(f, "{command}"),
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

fn write_plays<W: fmt::Write + ?Sized, const WIDTH: usize>(
    plays: &[Move],
    wrap: &mut Wrap<'_, W, WIDTH>,
    first_ply: usize,
    notation: Notation,
) -> fmt::Result {
    for (ply, (index, play)) in (first_ply..).zip(plays.iter().enumerate()) {
        if should_write_move_number(ply, index, plays) {
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
                Annotation::Valuation(valuation) => commands.push(valuation.command()),
                Annotation::Command(command) => commands.push(command.clone()),
            }
        }
        let mut comment = self.comment.clone().unwrap_or_default();
        comment.commands.extend(commands);
        if !comment.is_empty() {
            wrap.token(comment)?;
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
        write_plays(&variation.moves, wrap, ply, notation)?;
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

impl Comment {
    fn from_text(text: Text) -> Self {
        Self { commands: Commands::default(), text: Some(text) }
    }

    fn merge(&mut self, comment: &Comment) {
        self.commands.extend(comment.commands.iter().cloned());
        match (&mut self.text, &comment.text) {
            (Some(into), Some(comment)) => into.merge(comment),
            (None, Some(comment)) => self.text = Some(comment.clone()),
            _ => {}
        }
    }

    fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.text.is_none()
    }
}

impl fmt::Display for Comment {
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
            write!(f, "{text}")?;
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

fn should_write_move_number(ply: usize, index: usize, plays: &[Move]) -> bool {
    ply.is_multiple_of(2)
        || index == 0
        || plays[index - 1].has_intervening_annotation_or_variation()
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
        board::{Player, Role::*, Scharnagl},
        formats::san,
        position::Check,
        square::{File::*, Square::*},
    };

    use super::*;

    fn text(text: &str) -> Text {
        Text::new(text).unwrap()
    }

    fn comment(comment: &str) -> Comment {
        Comment::from_text(text(comment))
    }

    #[test]
    fn parse_intro_placeholder() {
        let pgn: Pgn = "1. -- {Introduction} *".parse().unwrap();
        assert!(pgn.moves.is_empty());
        assert_eq!(pgn.intro, Some(comment("Introduction")));
        let game = crate::Game::from_pgn(pgn.clone()).unwrap();
        assert_eq!(game.start().comment(), Some(&text("Introduction")));
    }

    #[test]
    fn parse_pgn() {
        let text = r#"
            [Event "Casual Game"]
            [Site "Berlin GER"]
            [Date "1852.??.??"]
            [Round "?"]
            [White "Adolf Anderssen"]
            [Black "Jean Dufresne"]
            [Result "1-0"]

            1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 {Italian Game} 1-0
        "#;

        let pgn = parse::pgn.parse(text).unwrap();
        assert_eq!(pgn.tags.len(), 7);
        assert_eq!(pgn.tags[0], Tag::Event("Casual Game".to_string()));
        assert_eq!(pgn.moves.len(), 6);
        assert_eq!(pgn.outcome, Some(Outcome::White(None)));
        assert_eq!(pgn.moves[5].comment, Some(comment("Italian Game")));
    }

    #[test]
    fn parse_tag_section() {
        let tags = Tags::from_pgn("[Event \"First glance\"]\n\nnot movetext").unwrap();
        assert_eq!(tags.as_slice(), [Tag::Event("First glance".to_string())]);
    }

    #[test]
    fn parse_pgn_without_termination_marker() {
        let pgn = pgn.parse("1. e4 e5").unwrap();

        assert_eq!(pgn.moves.len(), 2);
        assert_eq!(pgn.outcome, None);
    }

    #[test]
    fn parse_variation_and_nags() {
        let text = r#"[Event "x"]

1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *
"#;

        let pgn = pgn.parse(text).unwrap();
        let e4 = &pgn.moves[0];
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

        let e5 = &pgn.moves[1];
        assert_eq!(e5.annotations, vec![Annotation::Nag(Nag::Numeric(1))]);
        assert_eq!(e5.variations.len(), 1);
        assert_eq!(e5.variations[0].moves[0].comment, Some(comment("Sicilian")));
        assert_eq!(pgn.outcome, None);
    }

    #[test]
    fn convert_to_game() {
        let pgn = pgn
            .parse(
                r#"[Event "x"]

1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *
"#,
            )
            .unwrap();
        let game = crate::Game::from_pgn(pgn).unwrap();

        assert_eq!(game.roster().event, Some(text("x")));
        assert_eq!(game.tags().len(), 0);
        assert_eq!(game.start().options().len(), 1);

        let e4 = game.start().main().unwrap();
        assert_eq!(e4.play().to, E4);
        assert_eq!(e4.nags(), &[Nag::Symbol("!".to_string())]);
        assert_eq!(e4.position().options().len(), 2);

        let e5 = e4.position().main().unwrap();
        assert_eq!(e5.play().to, E5);
        assert_eq!(e5.nags(), &[Nag::Numeric(1)]);

        let c5 = e4.position().alternatives().next().unwrap();
        assert_eq!(c5.play().to, C5);
        assert_eq!(c5.comment(), Some(&text("Sicilian")));
    }

    #[test]
    fn title_roundtrip() {
        let pgn: Pgn = "[Title \"Sveshnikov \\\"study\\\"\"]\n\n*".parse().unwrap();
        let mut game = crate::Game::from_pgn(pgn).unwrap();
        assert_eq!(game.title(), Some(&text("Sveshnikov \"study\"")));
        assert!(game.tags().is_empty());

        game.set_title(Some(text("New title")));
        let exported = Pgn::from(game);
        assert!(exported.tags.contains(&Tag::Title("New title".to_string())));
        assert_eq!(crate::Game::from_pgn(exported).unwrap().title(), Some(&text("New title")));
    }

    #[test]
    fn convert_orientation_and_commands() {
        let pgn = pgn
            .parse(
                r#"[Orientation "BLACK"]

1. e4 {[%foo bar][%exp true][%eval 0.205,18]} (1. d4) 1... e5 {[%eval #-3,22]} *
"#,
            )
            .unwrap();
        let game = crate::Game::from_pgn(pgn).unwrap();

        assert_eq!(game.orientation(), Player::Black);
        let e4 = game.start().main().unwrap();
        assert!(e4.expanded());
        assert_eq!(e4.analysis(), None);
        assert_eq!(e4.position().commands().len(), 2);
        assert_eq!(e4.position().commands()[0].command.as_ref(), "foo");
        let e5 = e4.position().main().unwrap();
        assert_eq!(e5.analysis(), None);

        let pgn = Pgn::from(game).to_string();
        assert!(pgn.contains("[Orientation \"black\"]"), "{pgn}");
        let eval = pgn.find("[%eval 0.21,18]").unwrap();
        let expanded = pgn.find("[%exp true]").unwrap();
        let command = pgn.find("[%foo bar]").unwrap();
        assert!(expanded < command && command < eval, "{pgn}");
        assert!(pgn.contains("[%eval #-3,22]"), "{pgn}");
    }

    #[test]
    fn eval_policies() {
        let pgn: Pgn = r#"[Valuation "0.10,8"]

1. e4 {[%eval 0.30,12][%foo bar]} (1. d4 {[%eval -0.20,10]}) *"#
            .parse()
            .unwrap();
        let game = crate::Game::from_pgn(pgn.clone()).unwrap();
        let included = Pgn::from(game.clone());
        let dropped =
            crate::Game::from_pgn_with(pgn.clone(), &Import { eval: import::Eval::Drop }).unwrap();
        let omitted = Pgn::from(dropped);
        let text = omitted.to_string();

        assert!(included.to_string().contains("[Valuation \"0.10,8\"]"));
        assert!(included.to_string().contains("[%eval 0.30,12]"));
        assert!(!text.contains("[Valuation "), "{text}");
        assert!(!text.contains("%eval"), "{text}");
        assert!(text.contains("[%foo bar]"), "{text}");
        assert!(text.contains("d4"), "{text}");
        assert_eq!(Pgn::from(game), included);

        let pgn: Pgn = "1. e4 {[%eval #0]} *".parse().unwrap();
        let imported = crate::Game::from_pgn(pgn.clone()).unwrap();
        assert!(imported.start().main().unwrap().analysis().is_none());
        assert_eq!(imported.start().main().unwrap().position().commands().len(), 1);
        assert!(Pgn::from(imported).to_string().contains("[%eval #0]"));

        let dropped =
            crate::Game::from_pgn_with(pgn, &Import { eval: import::Eval::Drop }).unwrap();
        assert!(dropped.start().main().unwrap().analysis().is_none());
        assert!(dropped.start().main().unwrap().position().commands().is_empty());

        let malformed: Pgn = "1. e4 {[%eval unknown][%foo bar]} *".parse().unwrap();
        let dropped =
            crate::Game::from_pgn_with(malformed, &Import { eval: import::Eval::Drop }).unwrap();
        let play = dropped.start().main().unwrap();
        let position = play.position();
        let commands = position.commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].command.as_ref(), "foo");

        let analysis = position::Analysis::stalemate();
        let evaluation: Option<Eval> = (&analysis).into();
        assert_eq!(evaluation, None);
    }

    #[test]
    fn convert_fen_game() {
        let pgn = pgn
            .parse(
                r#"[FEN "4k3/8/8/8/8/8/4P3/4K3 w - - 0 1"]

1. e4 *
"#,
            )
            .unwrap();
        let game = crate::Game::from_pgn(pgn).unwrap();

        let e4 = game.start().main().unwrap();
        assert_eq!(e4.play().from, E2);
        assert_eq!(e4.play().to, E4);
    }

    #[test]
    fn use_last_fen_tag_for_start() {
        let pgn = pgn
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
    fn parse_variant_tag() {
        let text = r#"
            [Variant "Fischer Random"]
            *
        "#;
        let pgn = pgn.parse(text).unwrap();

        assert_eq!(pgn.tags, vec![Tag::Variant("Fischer Random".to_string())].into());
        assert_eq!(pgn.mode(), Freestyle);
        assert!(pgn.to_string().contains("[Variant \"Fischer Random\"]"));
    }

    #[test]
    fn prefer_freestyle_variant() {
        let text = r#"
            [Variant "Chess960"]
            [Variant "Standard"]
            *
        "#;
        let pgn = pgn.parse(text).unwrap();

        assert_eq!(pgn.mode(), Freestyle);
    }

    #[test]
    fn prefer_freestyle_over_unsupported_variant() {
        let text = r#"
            [Variant "Antichess"]
            [Variant "Chess960"]
            *
        "#;
        let pgn = pgn.parse(text).unwrap();

        assert_eq!(pgn.mode(), Freestyle);

        let text = r#"
            [Variant "Chess960"]
            [Variant "Antichess"]
            *
        "#;
        let pgn = parse::pgn.parse(text).unwrap();

        assert_eq!(pgn.mode(), Freestyle);
    }

    #[test]
    fn ignore_unsupported_variant_tags() {
        let text = r#"
            [Variant "Antichess"]
            *
        "#;
        let pgn = pgn.parse(text).unwrap();

        assert_eq!(pgn.mode(), Chess);
    }

    #[test]
    fn fall_back_to_position_castling() {
        let fen = r#"
            [FEN "8/8/8/8/8/8/8/8 w - - 0 1"]
            *
        "#;
        let pgn = parse::pgn.parse(fen).unwrap();

        assert_eq!(pgn.mode(), Chess);

        let fen = r#"
            [Variant "Standard"]
            [FEN "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9"]
            *
        "#;
        let pgn = parse::pgn.parse(fen).unwrap();

        assert_eq!(pgn.mode(), Freestyle);
        assert!(pgn.to_string().contains(" w HFhf - 2 9\"]"));
    }

    #[test]
    fn convert_to_game_mode() {
        let pgn = pgn.parse(r#"[Event "x"] 1. e4 *"#).unwrap();
        let chess = crate::Game::try_from(pgn).unwrap();
        assert_eq!(chess.mode(), Chess);

        let pgn = parse::pgn
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
    fn convert_unsupported_variant_as_chess() {
        let pgn = pgn.parse(r#"[Variant "Antichess"] *"#).unwrap();
        let game = crate::Game::try_from(pgn).unwrap();

        assert_eq!(game.mode(), Chess);
    }

    #[test]
    fn reject_invalid_start() {
        let pgn =
            pgn.parse(r#"[Variant "Standard"] [FEN "4k3/8/8/8/8/8/8/4K2P w - - 0 1"] *"#).unwrap();
        let error = crate::Game::try_from(pgn).err().unwrap();

        assert!(matches!(error, convert::Error::Start { mode: Chess, .. }));
    }

    #[test]
    fn display_fen_game_from_its_start_ply() {
        let fen = "4k3/8/8/8/8/8/4P3/4K3 b - - 0 17";
        let position = Position::from_fen(fen).unwrap();
        let mut game = crate::Game::chess(position).unwrap();
        game.start_mut().push(crate::Move::normal(King, E8, D8)).unwrap();

        let pgn = Pgn::from(game);
        assert!(pgn.to_string().contains("[SetUp \"1\"]"), "{}", pgn);
        assert!(pgn.to_string().contains(&format!("[FEN \"{fen}\"]")), "{}", pgn);
        assert!(pgn.to_string().contains("\n17... Kd8 *"), "{}", pgn);
        assert_eq!(pgn.movetext(), "17... Kd8");
    }

    #[test]
    fn display_figurine_movetext() {
        let mut game = crate::Game::chess(Position::start()).unwrap();
        let e4 = game.start_mut().into_push(crate::Move::normal(Pawn, E2, E4)).unwrap();
        e4.into_position().push(crate::Move::normal(Knight, G8, F6)).unwrap();

        let pgn = Pgn::from(game);
        assert_eq!(pgn.movetext(), "1. e4 Nf6");
        assert_eq!(pgn.figurine_movetext(), "1. e4 ♘f6");
    }

    #[test]
    fn convert_from_game() {
        let mut game = crate::Game::chess(Position::start()).unwrap();
        game.update_roster(|roster| roster.event = Some(text("x")));

        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().slot();
        game.start_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap();

        let mut e4 = game.play_mut(e4).unwrap();
        e4.update_nags(|nags| nags.push(Nag::Symbol("!".to_string())));
        let mut position = e4.into_position();
        position.push(crate::Move::normal(Pawn, E7, E5)).unwrap();
        let mut c5 = position.push(crate::Move::normal(Pawn, C7, C5)).unwrap();
        c5.set_comment(Some(text("Sicilian")));

        let pgn = Pgn::from(game);
        assert_eq!(
            pgn.to_string(),
            r#"[Event "x"]
[Result "*"]

1. e4! (1. d4) 1... e5 (1... c5 {Sicilian}) *"#
        );
    }

    #[test]
    fn convert_from_freestyle_game() {
        let game = crate::Game::freestyle(Position::freestyle(Scharnagl::CHESS));

        let pgn = Pgn::from(game);

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
    fn roundtrip_kiwipete_game_tree() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        let position = Position::from_fen(fen).unwrap();
        let mut game = crate::Game::chess(position).unwrap();

        for play in position.legal_plays() {
            let id = game.start_mut().push(play).unwrap().slot();
            let replies = game.play(id).unwrap().position().legal().to_vec();
            let mut position = game.play_mut(id).unwrap().into_position();
            for reply in replies {
                position.push(reply).unwrap();
            }
        }

        let pgn = Pgn::from(game);
        let game = crate::Game::from_pgn(pgn.clone()).unwrap();
        let roundtrip = Pgn::from(game);

        assert_eq!(roundtrip, pgn);
    }

    #[test]
    fn parse_annotations_between_variations() {
        let text = r#"[Event "x"]

1. e4 e5 {A} (1... c5) {B} (1... e6) {C} 2. Nf3 *
"#;

        let pgn = pgn.parse(text).unwrap();
        let e5 = &pgn.moves[1];
        assert_eq!(e5.comment, Some(comment("A")));
        assert_eq!(e5.variations.len(), 2);
        assert_eq!(e5.variations[0].outro, Some(comment("B")));
        assert_eq!(e5.variations[1].outro, Some(comment("C")));
        assert_eq!(
            pgn.to_string(),
            r#"[Event "x"]

1. e4 e5 {A} (1... c5) {B} (1... e6) {C} 2. Nf3 *"#
        );
    }

    #[test]
    fn merge_mainline_affixes_into_position_comments() {
        let mut game = crate::Game::from_pgn("{Root} 1. e4 {After} *".parse().unwrap()).unwrap();
        let slot = game.start().main().unwrap().slot();
        let mut play = game.play_mut(slot).unwrap();
        play.set_intro(Some(text("Intro")));
        play.set_outro(Some(text("Outro")));

        let pgn = Pgn::from(game);
        let output = pgn.to_string();
        assert!(output.contains("{Root\n\nIntro} 1. e4 {After\n\nOutro}"), "{output}");

        let game = crate::Game::from_pgn(pgn).unwrap();
        let play = game.start().main().unwrap();
        assert_eq!(game.start().comment(), Some(&text("Root\n\nIntro")));
        assert_eq!(play.comment(), Some(&text("After\n\nOutro")));
        assert_eq!(play.affixes(), &crate::game::Affixes::default());
    }

    #[test]
    fn extract_commands_from_move_comments() {
        let pgn =
            pgn.parse(r#"[Event "x"] 1. e4 {[%cal Ge2e4] [%clk 0:14:49] Good move.} *"#).unwrap();
        let e4 = &pgn.moves[0];
        let commands = &e4.comment.as_ref().unwrap().commands;
        assert_eq!(
            commands.colored_arrows().copied().collect::<Vec<_>>(),
            vec![game::Arrow { color: Some(game::Color::Green), from: E2, to: E4 }]
        );
        assert_eq!(
            commands[1],
            Command::Clock(command::Clock(command::Time {
                hours: 0,
                minutes: 14,
                seconds: 49,
                centiseconds: 0,
            }))
        );
        assert_eq!(e4.comment.as_ref().unwrap().text, Some(text("Good move.")));
        assert_eq!(
            pgn.to_string(),
            r#"[Event "x"]

1. e4 {[%cal Ge2e4] [%clk 0:14:49] Good move.} *"#
        );
    }

    #[test]
    fn clock_centiseconds() {
        let text = r#"1. e4 {[%clk 0:05:01.7] [%egt 0:05:01.07]} 1... e5 {[%emt 0:00:00.3] [%mct 09:05:00.03]} *"#;
        let pgn: Pgn = text.parse().unwrap();
        let output = pgn.to_string();
        assert!(output.contains("[%clk 0:05:01.7]"));
        assert!(output.contains("[%egt 0:05:01.07]"));
        assert!(output.contains("[%emt 0:00:00.3]"));
        assert!(output.contains("[%mct 09:05:00.03]"));
        assert_eq!(output.parse::<Pgn>().unwrap(), pgn);
    }

    #[test]
    fn extract_multi_parameter_commands() {
        let pgn = pgn
            .parse(r#"[Event "x"] 1. e4 {[%tqu "En","find the move","","","e2e4","",10]} *"#)
            .unwrap();
        let e4 = &pgn.moves[0];
        assert_eq!(
            e4.comment.as_ref().unwrap().commands,
            Commands(vec![Command::Other(command::Other {
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
            })])
        );
        assert_eq!(
            pgn.to_string(),
            r#"[Event "x"]

1. e4 {[%tqu "En","find the move","","","e2e4","",10]} *"#
        );
    }

    #[test]
    fn extract_commands_with_trailing_whitespace() {
        let pgn = pgn.parse(r#"[Event "x"] 1. e4 {[%foo ]} *"#).unwrap();
        assert_eq!(
            pgn.moves[0].comment.as_ref().unwrap().commands,
            Commands(vec![Command::Other(command::Other {
                command: text("foo"),
                parameters: vec![],
            })])
        );
        assert_eq!(
            pgn.to_string(),
            r#"[Event "x"]

1. e4 {[%foo]} *"#
        );
    }

    #[test]
    fn ignore_empty_comments() {
        let pgn = pgn.parse(r#"[Event "x"] 1. e4 {} {   } e5 *"#).unwrap();
        assert_eq!(pgn.moves[0].comment, None);
        assert_eq!(pgn.moves[1].comment, None);
    }

    #[test]
    fn parse_san_in_moves() {
        let pgn = pgn.parse(r#"[Event "x"] 1. exd8=Q# *"#).unwrap();
        assert_eq!(
            pgn.moves[0].san,
            san::San {
                play: san::Move::Normal {
                    role: Pawn,
                    file: Some(E),
                    rank: None,
                    capture: true,
                    to: D8,
                    promotion: Some(Queen),
                },
                check: Some(Check::Checkmate),
            }
        );
    }

    #[test]
    fn display_game() {
        let pgn = pgn.parse(r#"[Event "x"] 1. e4! e5 $1 (1... c5 {Sicilian}) 2. Nf3 *"#).unwrap();
        assert_eq!(
            pgn.to_string(),
            "[Event \"x\"]\n\n1. e4! 1... e5 $1 (1... c5 {Sicilian}) 2. Nf3 *"
        );
    }

    #[test]
    fn read_pgns_one_at_a_time() {
        let input = b"[Event \"a\"]\n1. e4 *\n\n[Event \"b\"]\n1. d4 *\n";
        let pgns = stream::pgns(&input[..]).map(|pgn| pgn.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(pgns.len(), 2);
        assert_eq!(pgns[0].tags[0], Tag::Event("a".to_string()));
        assert_eq!(pgns[1].tags[0], Tag::Event("b".to_string()));
    }

    #[test]
    fn read_comment_without_splitting_pgn() {
        let input = b"[Event \"a\"]\n1. e4 {\n[not a tag]\n} *\n\n[Event \"b\"]\n1. d4 *\n";
        let games = stream::pgns(&input[..]).map(|pgn| pgn.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].moves[0].comment, Some(comment("[not a tag]")));
    }

    #[test]
    fn read_movetext_after_tag_on_same_line() {
        let input = b"[Event \"a\"] 1. e4 *\n\n[Event \"b\"] 1. d4 *\n";
        let games = stream::pgns(&input[..]).map(|pgn| pgn.unwrap().unwrap()).collect::<Vec<_>>();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].tags[0], Tag::Event("a".to_string()));
        assert_eq!(games[1].tags[0], Tag::Event("b".to_string()));
    }

    #[test]
    fn parse_line_wrapped_black_move_without_number() {
        pgn.parse("[Event \"x\"]\n\n14. f3 b6\n15. Be2 *").unwrap();
    }
}
