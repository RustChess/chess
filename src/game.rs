use core::{fmt, num::NonZeroU32, ops::Deref, str::FromStr};

use std::collections::{BTreeMap as Map, BTreeSet as Set};

use crate as chess;
use crate::{
    Player, Square,
    formats::Text,
    position::{self, Analysis, Check, ScoredLine, Vec1},
    square::{File, Rank},
};

pub use crate::formats::pgn::{command, tag};

use self::layout::Layout as _;

// #[cfg(feature = "serde")]
// pub mod storage;

pub mod change;
pub mod options;
pub mod traversal;

mod cursor;
mod layout;
mod merge;
pub(in crate::game) mod tree;

/// An error from accessing or editing a game tree.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("games have different starting positions or modes")]
    DifferentStart,
    #[error(transparent)]
    Start(#[from] start::Error),
    #[error(transparent)]
    Traversal(#[from] traversal::Error),
    #[error(transparent)]
    Options(#[from] options::Error),
    #[error(transparent)]
    Change(#[from] change::Error),
}

/// A result of editing a game tree.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Errors from the rooted boundary of a game tree.
pub mod start {
    /// An error from requesting an incoming move at the start position.
    #[derive(Debug, thiserror::Error)]
    #[error("start position has no incoming move")]
    pub struct Error;

    /// A result of accessing an incoming move.
    pub type Result<T, E = Error> = core::result::Result<T, E>;
}

/// A game tree and its game-level data.
#[derive(Clone, PartialEq)]
pub struct Game {
    mode: Mode,
    changes: Option<Vec<change::Change>>,
    tree: tree::Tree,
    public: GamePublic,
}

/// A position followed by a linear sequence of moves.
#[derive(Clone, Debug, PartialEq)]
pub struct Line<P, M> {
    pub position: P,
    pub moves: Vec<M>,
}

/// Public data associated with a game.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub struct GamePublic {
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub title: Option<Text>,
    pub orientation: Player,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<Outcome>,
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub roster: Roster,
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub tags: Vec<tag::Other>,
}

impl Default for GamePublic {
    fn default() -> Self {
        Self {
            title: None,
            orientation: Player::White,
            outcome: None,
            roster: Roster::default(),
            tags: Vec::new(),
        }
    }
}

/// The allocated slot of a position within a game.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[serde(from = "u32", into = "u32")]
pub enum PositionSlot {
    #[default]
    Start,
    Move(MoveSlot),
}

/// The allocated slot of a move within a game.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[serde(transparent)]
pub struct MoveSlot(NonZeroU32);

/// The starting position slot.
pub const START: PositionSlot = PositionSlot::Start;

/// A position in a game, with its derived, analysis, and public data.
#[derive(Clone, PartialEq)]
pub struct Position {
    // pointers right
    options: Vec<MoveSlot>,

    // core datum
    position: chess::Position,

    // derived data
    check: Option<Check>,
    legal: Vec<chess::Move>,

    // analysis data
    analysis: Option<Analysis>,

    // public data
    pub public: PositionPublic,
}

/// Public data associated with a position.
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct PositionPublic {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub arrows: Vec<Arrow>,
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub comment: Option<Text>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<command::Other>,
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub expanded: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<Marker>,
}

/// A move in a game, with its derived and public data.
#[derive(Clone, PartialEq)]
pub struct Move {
    // pointer left
    previous: PositionSlot,

    // core datum
    play: chess::Move,

    // derived data
    short: Short,

    pub public: MovePublic,
}

/// Public data associated with a move.
#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
pub struct MovePublic {
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub affixes: Affixes,
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub nags: Vec<Nag>,
}

/// An immutable reference to a position in a game.
#[derive(Clone, Copy)]
pub struct PositionRef<'g> {
    game: &'g Game,
    slot: PositionSlot,
}

/// A mutable reference to a position in a game.
pub struct PositionMut<'g> {
    game: &'g mut Game,
    slot: PositionSlot,
    protected: Protected,
}

/// An immutable reference to a move in a game.
#[derive(Clone, Copy)]
pub struct MoveRef<'g> {
    game: &'g Game,
    slot: MoveSlot,
}

/// A mutable reference to a move in a game.
pub struct MoveMut<'g> {
    game: &'g mut Game,
    slot: MoveSlot,
    protected: Protected,
}

/// A movable position within a game tree.
#[derive(Clone, PartialEq)]
pub struct Cursor<G = Game> {
    game: G,
    slot: PositionSlot,
    preserve: Set<PositionSlot>,
}

/// A cursor over an immutably borrowed game.
pub type CursorRef<'g> = Cursor<&'g Game>;

/// A cursor over a mutably borrowed game.
pub type CursorMut<'g> = Cursor<&'g mut Game>;

/// Comments surrounding an option in PGN movetext.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Affixes {
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub intro: Option<Text>,
    #[serde(default, skip_serializing_if = "macros::is_default")]
    pub outro: Option<Text>,
}

#[derive(Default)]
struct Ambiguity {
    exists: bool,
    file: bool,
    rank: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Arrow {
    pub from: Square,
    pub to: Square,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    Red,
    Green,
    Blue,
    Yellow,
}

/// A detached, injective mapping from one slot allocation to another.
///
/// Moreover, each new move slot is greater than the slot of the position containing it as an option.
pub struct Reslot(Map<MoveSlot, MoveSlot>);

/// Canonical tree-slot assignment order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Layout {
    /// The move encounter order used while constructing a game from PGN.
    ///
    /// For `1. e4 (1. d4 d5) e5`, slots are assigned to `e4`, `d4`, `d5`, then `e5`
    /// consecutively as 1, 2, 3, 4.
    Pgn,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Marker {
    pub square: Square,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Chess,
    Freestyle,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(untagged)]
pub enum Nag {
    Numeric(u32),
    Symbol(String),
}

#[derive(
    Clone, Copy, Debug, DeserializeFromStr, Eq, Ord, PartialEq, PartialOrd, SerializeDisplay,
)]
#[non_exhaustive]
pub enum Outcome {
    White(Option<Win>),
    Black(Option<Win>),
    Draw(Option<Draw>),
}

#[derive(
    Clone, Copy, Debug, DeserializeFromStr, Eq, Ord, PartialEq, PartialOrd, SerializeDisplay,
)]
#[non_exhaustive]
pub enum Win {
    Checkmate,
    Resignation,
    FlagFall,
    Forfeit,
    Abandonment,
}

#[derive(
    Clone, Copy, Debug, DeserializeFromStr, Eq, Ord, PartialEq, PartialOrd, SerializeDisplay,
)]
#[non_exhaustive]
pub enum Draw {
    Agreement,
    Stalemate,
    DeadPosition,
    ThreefoldRepetition,
    FivefoldRepetition,
    Repetition,
    FiftyMoves,
    SeventyFiveMoves,
    FlagFall,
}

impl Outcome {
    pub const fn result(self) -> &'static str {
        match self {
            Self::White(_) => "1-0",
            Self::Black(_) => "0-1",
            Self::Draw(_) => "1/2-1/2",
        }
    }

    pub fn from_result(result: &str) -> Option<Self> {
        use Outcome::*;
        Some(match result {
            "1-0" => White(None),
            "0-1" => Black(None),
            "1/2-1/2" => Draw(None),
            _ => return None,
        })
    }

    pub const fn winner(self) -> Option<Player> {
        use Outcome::*;
        match self {
            White(_) => Some(Player::White),
            Black(_) => Some(Player::Black),
            Draw(_) => None,
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Outcome::*;
        match self {
            White(None) => f.write_str("white:win"),
            White(Some(reason)) => write!(f, "white:win:{reason}"),
            Black(None) => f.write_str("black:win"),
            Black(Some(reason)) => write!(f, "black:win:{reason}"),
            Draw(None) => f.write_str("draw"),
            Draw(Some(reason)) => write!(f, "draw:{reason}"),
        }
    }
}

impl FromStr for Outcome {
    type Err = &'static str;

    fn from_str(value: &str) -> core::result::Result<Self, Self::Err> {
        use Outcome::*;
        Ok(if value == "white:win" {
            White(None)
        } else if let Some(reason) = value.strip_prefix("white:win:") {
            White(Some(reason.parse()?))
        } else if value == "black:win" {
            Black(None)
        } else if let Some(reason) = value.strip_prefix("black:win:") {
            Black(Some(reason.parse()?))
        } else if value == "draw" {
            Draw(None)
        } else if let Some(reason) = value.strip_prefix("draw:") {
            Draw(Some(reason.parse()?))
        } else {
            return Err("unknown game outcome");
        })
    }
}

impl fmt::Display for Win {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Win::*;
        f.write_str(match self {
            Checkmate => "checkmate",
            Resignation => "resignation",
            FlagFall => "flag-fall",
            Forfeit => "forfeit",
            Abandonment => "abandonment",
        })
    }
}

impl FromStr for Win {
    type Err = &'static str;

    fn from_str(value: &str) -> core::result::Result<Self, Self::Err> {
        use Win::*;
        Ok(match value {
            "checkmate" => Checkmate,
            "resignation" => Resignation,
            "flag-fall" => FlagFall,
            "forfeit" => Forfeit,
            "abandonment" => Abandonment,
            _ => return Err("unknown win reason"),
        })
    }
}

impl fmt::Display for Draw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Draw::*;
        f.write_str(match self {
            Agreement => "agreement",
            Stalemate => "stalemate",
            DeadPosition => "dead-position",
            ThreefoldRepetition => "threefold-repetition",
            FivefoldRepetition => "fivefold-repetition",
            Repetition => "repetition",
            FiftyMoves => "fifty-moves",
            SeventyFiveMoves => "seventy-five-moves",
            FlagFall => "flag-fall",
        })
    }
}

impl FromStr for Draw {
    type Err = &'static str;

    fn from_str(value: &str) -> core::result::Result<Self, Self::Err> {
        use Draw::*;
        Ok(match value {
            "agreement" => Agreement,
            "stalemate" => Stalemate,
            "dead-position" => DeadPosition,
            "threefold-repetition" => ThreefoldRepetition,
            "fivefold-repetition" => FivefoldRepetition,
            "repetition" => Repetition,
            "fifty-moves" => FiftyMoves,
            "seventy-five-moves" => SeventyFiveMoves,
            "flag-fall" => FlagFall,
            _ => return Err("unknown draw reason"),
        })
    }
}

/// Positions retained by attached mutable values.
#[derive(Clone)]
pub struct Protected {
    pub unremovable: Set<PositionSlot>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Roster {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub white: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub black: Option<Text>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Short {
    pub file: Option<File>,
    pub rank: Option<Rank>,
}

impl Game {
    pub fn new(position: chess::Position) -> Self {
        let mode = position.requires_freestyle().into();
        Self::unchecked_with_mode(position, mode)
    }

    pub fn with_mode(position: chess::Position, mode: Mode) -> Option<Self> {
        (mode.is_freestyle() || !position.requires_freestyle())
            .then(|| Self::unchecked_with_mode(position, mode))
    }

    pub fn chess(position: chess::Position) -> Option<Self> {
        Self::with_mode(position, Mode::Chess)
    }

    pub fn freestyle(position: chess::Position) -> Self {
        Self::unchecked_with_mode(position, Mode::Freestyle)
    }

    /// Reassigns move slots in a prescribed layout and returns the old-to-new mapping.
    ///
    /// Tracked changes are discarded, as they use the the old slots.
    /// Change tracking remains enabled or disabled as configured.
    pub fn reslot(&mut self, layout: Layout) -> Reslot {
        let reslot = layout.reslot(self);
        self.tree.reslot(&reslot);
        if let Some(changes) = self.changes.as_mut() {
            changes.clear();
        }
        reslot
    }

    pub const fn mode(&self) -> Mode {
        self.mode
    }

    pub const fn tracks_changes(&self) -> bool {
        self.changes.is_some()
    }

    pub fn track_changes(&mut self, on: bool) {
        if on {
            self.changes.get_or_insert_default();
        } else {
            self.changes = None;
        }
    }

    pub fn changes(&self) -> Option<&[change::Change]> {
        self.changes.as_deref()
    }

    pub fn revisions(&self) -> Option<usize> {
        self.changes.as_ref().map(Vec::len)
    }

    pub fn clear_analyses(&mut self) {
        self.tree.clear_analyses();
    }

    pub const fn public(&self) -> &GamePublic {
        &self.public
    }

    pub const fn title(&self) -> Option<&Text> {
        self.public.title.as_ref()
    }

    pub const fn orientation(&self) -> Player {
        self.public.orientation
    }

    pub const fn outcome(&self) -> Option<Outcome> {
        self.public.outcome
    }

    pub const fn roster(&self) -> &Roster {
        &self.public.roster
    }

    pub const fn tags(&self) -> &[tag::Other] {
        self.public.tags.as_slice()
    }

    pub fn set_orientation(&mut self, orientation: Player) {
        self.update_public(|public| public.orientation = orientation);
    }

    pub fn set_title(&mut self, title: Option<Text>) {
        self.update_public(|public| public.title = title);
    }

    pub fn set_outcome(&mut self, outcome: Option<Outcome>) {
        self.update_public(|public| public.outcome = outcome);
    }

    pub fn update_roster(&mut self, update: impl FnOnce(&mut Roster)) {
        self.update_public(|public| update(&mut public.roster));
    }

    pub fn update_tags(&mut self, update: impl FnOnce(&mut Vec<tag::Other>)) {
        self.update_public(|public| update(&mut public.tags));
    }

    pub fn update_public(&mut self, update: impl FnOnce(&mut GamePublic)) {
        let before = self.tracks_changes().then(|| self.public.clone());
        update(&mut self.public);
        if let Some(before) = before
            && before != self.public
        {
            self.track(change::Patch::Game(Box::new(before)));
        }
    }

    fn unchecked_with_mode(position: chess::Position, mode: Mode) -> Self {
        Self::unchecked_with_mode_next(position, mode, MoveSlot::MIN)
    }

    fn unchecked_with_mode_next(position: chess::Position, mode: Mode, next: MoveSlot) -> Self {
        Self {
            mode,
            changes: None,
            tree: tree::Tree::new(position, next),
            public: Default::default(),
        }
    }
}

impl Position {
    fn new(position: chess::Position) -> Self {
        let legal = position.legal_plays();
        let check = Check::new(position.is_check(), legal.is_empty());

        Self {
            options: Vec::new(),
            position,
            check,
            legal,
            analysis: None,
            public: PositionPublic::default(),
        }
    }

    pub const fn arrows(&self) -> &[Arrow] {
        self.public.arrows.as_slice()
    }

    pub const fn options(&self) -> &[MoveSlot] {
        self.options.as_slice()
    }

    pub const fn position(&self) -> chess::Position {
        self.position
    }

    pub fn comment(&self) -> Option<&Text> {
        self.public.comment.as_ref()
    }

    pub const fn commands(&self) -> &[command::Other] {
        self.public.commands.as_slice()
    }

    pub const fn analysis(&self) -> Option<&Analysis> {
        self.analysis.as_ref()
    }

    pub const fn expanded(&self) -> bool {
        self.public.expanded
    }

    pub const fn legal(&self) -> &[chess::Move] {
        self.legal.as_slice()
    }

    pub const fn markers(&self) -> &[Marker] {
        self.public.markers.as_slice()
    }

    pub const fn check(&self) -> Option<Check> {
        self.check
    }

    pub const fn is_terminal(&self) -> bool {
        self.legal.is_empty()
    }

    pub const fn terminal(&self) -> Option<position::Terminal> {
        position::Terminal::new(self.check.is_some(), self.is_terminal())
    }

    pub const fn is_checkmate(&self) -> bool {
        matches!(self.terminal(), Some(position::Terminal::Checkmate))
    }

    pub const fn is_stalemate(&self) -> bool {
        matches!(self.terminal(), Some(position::Terminal::Stalemate))
    }
}

impl Deref for Position {
    type Target = chess::Position;

    fn deref(&self) -> &chess::Position {
        &self.position
    }
}

impl Move {
    pub const fn affixes(&self) -> &Affixes {
        &self.public.affixes
    }

    pub const fn nags(&self) -> &[Nag] {
        self.public.nags.as_slice()
    }

    pub const fn previous(&self) -> PositionSlot {
        self.previous
    }

    pub const fn play(&self) -> chess::Move {
        self.play
    }

    pub const fn short(&self) -> Short {
        self.short
    }
}

impl Deref for Move {
    type Target = chess::Move;

    fn deref(&self) -> &chess::Move {
        &self.play
    }
}

impl Ambiguity {
    fn consider(mut self, play: chess::Move, other: chess::Move) -> Self {
        self.exists = true;
        self.file |= other.from.file() == play.from.file();
        self.rank |= other.from.rank() == play.from.rank();
        self
    }

    fn file_resolves(&self) -> bool {
        self.exists && !self.file
    }

    fn rank_resolves(&self) -> bool {
        self.exists && !self.rank
    }
}

impl<M> position::Line<M> {
    pub fn resolve<N>(
        &self,
        position: PositionRef<'_>,
        resolver: impl Fn(PositionRef<'_>, &[M]) -> Vec<N>,
    ) -> Option<position::Line<N>> {
        let moves = resolver(position, self.as_slice());
        if moves.len() != self.len() {
            return None;
        }
        Some(position::Line(Vec1::new(moves).expect("source line has moves")))
    }
}

impl<M> ScoredLine<M> {
    pub fn resolve<N>(
        &self,
        position: PositionRef<'_>,
        resolver: impl Fn(PositionRef<'_>, &[M]) -> Vec<N>,
    ) -> Option<ScoredLine<N>> {
        Some(ScoredLine { line: self.line.resolve(position, resolver)?, score: self.score })
    }
}

impl<M> Vec1<ScoredLine<M>> {
    pub fn resolve<N>(
        &self,
        position: PositionRef<'_>,
        resolver: impl Fn(PositionRef<'_>, &[M]) -> Vec<N> + Copy,
    ) -> Option<Vec1<ScoredLine<N>>> {
        let resolved =
            self.iter().map(|line| line.resolve(position, resolver)).collect::<Option<_>>()?;
        Some(Vec1::new(resolved).expect("evaluation has lines"))
    }
}

impl Mode {
    pub const fn is_freestyle(self) -> bool {
        matches!(self, Self::Freestyle)
    }
}

impl From<bool> for Mode {
    fn from(freestyle: bool) -> Self {
        if freestyle { Self::Freestyle } else { Self::Chess }
    }
}

impl PositionSlot {
    pub const fn is_start(&self) -> bool {
        matches!(self, Self::Start)
    }
}

impl MoveSlot {
    pub const MIN: Self = Self(NonZeroU32::MIN);
    pub const MAX: Self = Self(NonZeroU32::MAX);

    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(slot) => Some(Self(slot)),
            None => None,
        }
    }

    pub const fn prev(self) -> Option<Self> {
        match NonZeroU32::new(self.0.get() - 1) {
            Some(slot) => Some(Self(slot)),
            None => None,
        }
    }

    /// Maps iterator.enumerate() indices 0, 1, 2...
    /// to Self::MIN = 1, Self::MIN + 1 = 2, etc.
    pub fn from_index(index: usize) -> Option<Self> {
        let slot = u32::try_from(index).ok()?.checked_add(1)?;
        Some(Self(NonZeroU32::new(slot)?))
    }
}

impl fmt::Display for MoveSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for MoveSlot {
    type Err = <NonZeroU32 as FromStr>::Err;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

impl fmt::Display for PositionSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", u32::from(*self))
    }
}

impl FromStr for PositionSlot {
    type Err = <u32 as FromStr>::Err;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        s.parse::<u32>().map(Self::from)
    }
}

impl From<MoveSlot> for PositionSlot {
    fn from(slot: MoveSlot) -> Self {
        Self::Move(slot)
    }
}

impl From<NonZeroU32> for MoveSlot {
    fn from(slot: NonZeroU32) -> Self {
        Self(slot)
    }
}

impl From<MoveSlot> for NonZeroU32 {
    fn from(slot: MoveSlot) -> Self {
        slot.0
    }
}

impl From<u32> for PositionSlot {
    fn from(slot: u32) -> Self {
        match NonZeroU32::new(slot) {
            Some(slot) => Self::Move(MoveSlot(slot)),
            None => Self::Start,
        }
    }
}

impl From<PositionSlot> for u32 {
    fn from(slot: PositionSlot) -> Self {
        match slot {
            PositionSlot::Move(slot) => slot.0.get(),
            PositionSlot::Start => 0,
        }
    }
}

impl Reslot {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (MoveSlot, MoveSlot)> + '_ {
        self.0.iter().map(|(&before, &after)| (before, after))
    }

    pub fn play(&self, slot: MoveSlot) -> Option<MoveSlot> {
        self.0.get(&slot).copied()
    }

    pub fn position(&self, slot: PositionSlot) -> Option<PositionSlot> {
        match slot {
            PositionSlot::Move(slot) => self.play(slot).map(Into::into),
            PositionSlot::Start => Some(PositionSlot::Start),
        }
    }
}

impl Default for Protected {
    fn default() -> Self {
        Self::new()
    }
}

impl Protected {
    pub const fn new() -> Self {
        Self { unremovable: Set::new() }
    }

    pub fn with_unremovable(&self, slot: PositionSlot) -> Self {
        let mut protected = self.clone();
        protected.unremovable.insert(slot);
        protected
    }
}

impl Short {
    pub fn new(legal: &[chess::Move], play: chess::Move) -> Self {
        let mut short = Short::default();

        if play.role == crate::Role::Pawn || play.is_castle() {
            return short;
        }

        let different_play = |other: &chess::Move| *other != play;
        let same_role_and_to = |other: &chess::Move| (other.role, other.to) == (play.role, play.to);
        let ambiguity = legal
            .iter()
            .copied()
            .filter(different_play)
            .filter(same_role_and_to)
            .fold(Ambiguity::default(), |ambiguity, other| ambiguity.consider(play, other));

        if ambiguity.file_resolves() {
            short.file = Some(play.from.file());
        } else if ambiguity.rank_resolves() {
            short.rank = Some(play.from.rank());
        } else if ambiguity.exists {
            short.file = Some(play.from.file());
            short.rank = Some(play.from.rank());
        }

        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_sorts_before_moves() {
        // Tree reslot derives the next move slot from the greatest position slot.
        assert!(PositionSlot::Start < PositionSlot::Move(MoveSlot::MIN));
    }

    #[test]
    fn position_slots_are_u32s() {
        for (position, integer) in [
            (PositionSlot::Start, 0),
            (PositionSlot::Move(MoveSlot::MIN), 1),
            (PositionSlot::Move(MoveSlot::MAX), u32::MAX),
        ] {
            assert_eq!(PositionSlot::from(integer), position);
            assert_eq!(u32::from(position), integer);
        }
    }

    #[test]
    fn move_slots_are_nonzero_u32s() {
        for integer in [NonZeroU32::MIN, NonZeroU32::MAX] {
            let slot = MoveSlot::from(integer);
            assert_eq!(NonZeroU32::from(slot), integer);
        }
    }

    #[test]
    fn slots_are_numeric_text() {
        let play = MoveSlot::from_index(21).unwrap();

        assert_eq!(play.to_string(), "22");
        assert_eq!("22".parse::<MoveSlot>().unwrap(), play);
        assert!("0".parse::<MoveSlot>().is_err());

        assert_eq!(PositionSlot::Start.to_string(), "0");
        assert_eq!("0".parse::<PositionSlot>().unwrap(), PositionSlot::Start);
        assert_eq!(PositionSlot::Move(play).to_string(), "22");
        assert_eq!("22".parse::<PositionSlot>().unwrap(), PositionSlot::Move(play));
    }

    #[test]
    fn outcome_text() {
        use Draw::*;

        for (outcome, text) in [
            (Outcome::White(None), "white:win"),
            (Outcome::White(Some(Win::Checkmate)), "white:win:checkmate"),
            (Outcome::White(Some(Win::Resignation)), "white:win:resignation"),
            (Outcome::White(Some(Win::FlagFall)), "white:win:flag-fall"),
            (Outcome::White(Some(Win::Forfeit)), "white:win:forfeit"),
            (Outcome::Black(Some(Win::Abandonment)), "black:win:abandonment"),
            (Outcome::Draw(None), "draw"),
            (Outcome::Draw(Some(Agreement)), "draw:agreement"),
            (Outcome::Draw(Some(Stalemate)), "draw:stalemate"),
            (Outcome::Draw(Some(DeadPosition)), "draw:dead-position"),
            (Outcome::Draw(Some(ThreefoldRepetition)), "draw:threefold-repetition"),
            (Outcome::Draw(Some(FivefoldRepetition)), "draw:fivefold-repetition"),
            (Outcome::Draw(Some(Repetition)), "draw:repetition"),
            (Outcome::Draw(Some(FiftyMoves)), "draw:fifty-moves"),
            (Outcome::Draw(Some(SeventyFiveMoves)), "draw:seventy-five-moves"),
            (Outcome::Draw(Some(FlagFall)), "draw:flag-fall"),
        ] {
            assert_eq!(outcome.to_string(), text);
            assert_eq!(text.parse(), Ok(outcome));
            assert_eq!(serde_json::to_string(&outcome).unwrap(), format!("\"{text}\""));
        }
    }

    #[test]
    fn outcome_results() {
        use Outcome::*;

        for (outcome, result, winner) in [
            (White(None), "1-0", Some(Player::White)),
            (Black(None), "0-1", Some(Player::Black)),
            (Draw(None), "1/2-1/2", None),
        ] {
            assert_eq!(outcome.result(), result);
            assert_eq!(Outcome::from_result(result), Some(outcome));
            assert_eq!(outcome.winner(), winner);
        }
        assert_eq!(Outcome::from_result("*"), None);
    }

    #[test]
    fn slots_are_json_numbers() {
        let play = MoveSlot::from_index(21).unwrap();

        assert_eq!(serde_json::to_string(&play).unwrap(), "22");
        assert_eq!(serde_json::from_str::<MoveSlot>("22").unwrap(), play);

        assert_eq!(serde_json::to_string(&PositionSlot::Start).unwrap(), "0");
        assert_eq!(serde_json::from_str::<PositionSlot>("0").unwrap(), PositionSlot::Start);
        assert_eq!(serde_json::to_string(&PositionSlot::Move(play)).unwrap(), "22");
        assert_eq!(serde_json::from_str::<PositionSlot>("22").unwrap(), PositionSlot::Move(play));
    }
}
