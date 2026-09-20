use core::{fmt, num::NonZeroU32, ops::Deref, str::FromStr};

use std::collections::{BTreeMap as Map, BTreeSet as Set};

use crate as chess;
use crate::{
    Player,
    formats::{Text, san::Check},
    square::{File, Rank},
};

use self::layout::Layout as _;

// #[cfg(feature = "serde")]
// pub mod storage;

pub mod change;
pub mod options;
pub mod traversal;

mod cursor;
mod layout;
pub(in crate::game) mod tree;

/// An error from accessing or editing a game tree.
#[derive(Debug, thiserror::Error)]
pub enum Error {
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

/// Public data associated with a game.
#[derive(Clone, PartialEq)]
pub struct GamePublic {
    pub orientation: Player,
    pub outcome: Outcome,
    pub roster: Roster,
    pub tags: Vec<Tag>,
}

impl Default for GamePublic {
    fn default() -> Self {
        Self {
            orientation: Player::White,
            outcome: Outcome::default(),
            roster: Roster::default(),
            tags: Vec::new(),
        }
    }
}

/// The stable identity of a position within a game.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[serde(from = "u32", into = "u32")]
pub enum PositionId {
    #[default]
    Start,
    Move(MoveId),
}

/// The stable identity of a move within a game.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[serde(transparent)]
pub struct MoveId(NonZeroU32);

/// The starting position identity.
pub const START: PositionId = PositionId::Start;

/// A position in a game, with its derived and public data.
#[derive(Clone, PartialEq)]
pub struct Position {
    // pointers right
    options: Vec<MoveId>,

    // core datum
    position: chess::Position,

    // derived data
    check: Option<Check>,
    legal: Vec<chess::Move>,

    pub public: PositionPublic,
}

/// Public data associated with a position.
#[derive(Clone, Default, PartialEq)]
pub struct PositionPublic {
    pub comment: Option<Text>,
    pub valuation: Option<Valuation>,
    pub expanded: bool,
}

/// A move in a game, with its derived and public data.
#[derive(Clone, PartialEq)]
pub struct Move {
    // pointer left
    previous: PositionId,

    // core datum
    play: chess::Move,

    // derived data
    short: Short,

    pub public: MovePublic,
}

/// Public data associated with a move.
#[derive(Clone, Default, PartialEq)]
pub struct MovePublic {
    pub affixes: Affixes,
    pub commands: Vec<Command>,
    pub nags: Vec<Nag>,
}

/// An immutable reference to a position in a game.
#[derive(Clone, Copy)]
pub struct PositionRef<'g> {
    game: &'g Game,
    id: PositionId,
}

/// A mutable reference to a position in a game.
pub struct PositionMut<'g> {
    game: &'g mut Game,
    id: PositionId,
    protected: Protected,
}

/// An immutable reference to a move in a game.
#[derive(Clone, Copy)]
pub struct MoveRef<'g> {
    game: &'g Game,
    id: MoveId,
}

/// A mutable reference to a move in a game.
pub struct MoveMut<'g> {
    game: &'g mut Game,
    id: MoveId,
    protected: Protected,
}

/// A movable position within a game tree.
#[derive(Clone, PartialEq)]
pub struct Cursor<G = Game> {
    game: G,
    id: PositionId,
    preserve: Set<PositionId>,
}

/// A cursor over an immutably borrowed game.
pub type CursorRef<'g> = Cursor<&'g Game>;

/// A cursor over a mutably borrowed game.
pub type CursorMut<'g> = Cursor<&'g mut Game>;

/// Comments surrounding an option in PGN movetext.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Affixes {
    pub intro: Option<Text>,
    pub outro: Option<Text>,
}

#[derive(Default)]
struct Ambiguity {
    exists: bool,
    file: bool,
    rank: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Command {
    pub command: Text,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<String>,
}

/// A detached, injective mapping from every pre-compaction move ID to its compact ID.
///
/// Moreover, each compact move ID is greater than the ID of the position containing it as an option.
pub struct Compact(Map<MoveId, MoveId>);

/// Canonical tree-slot assignment order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Layout {
    /// The move encounter order used while constructing a game from PGN.
    ///
    /// For `1. e4 (1. d4 d5) e5`, IDs are assigned to `e4`, `d4`, `d5`, then `e5`
    /// consecutively as 1, 2, 3, 4.
    Pgn,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Valuation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    #[serde(flatten)]
    pub score: Score,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Chess,
    Freestyle,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Nag {
    Numeric(u32),
    Symbol(String),
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub enum Outcome {
    White,
    Black,
    Draw,
    #[default]
    Unknown,
}

/// Positions retained by attached mutable values.
#[derive(Clone)]
pub struct Protected {
    pub unremovable: Set<PositionId>,
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Score {
    Centipawns(i32),
    Mate(i32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Short {
    pub file: Option<File>,
    pub rank: Option<Rank>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct Tag {
    pub key: Text,
    pub value: String,
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

    /// Reassigns move IDs in a prescribed layout and returns the old-to-new mapping.
    ///
    /// Tracked changes are discarded, as they use the the old IDs.
    /// Change tracking remains enabled or disabled as configured.
    pub fn compact(&mut self, layout: Layout) -> Compact {
        let compact = layout.compact(self);
        self.tree.compact(&compact);
        if let Some(changes) = self.changes.as_mut() {
            changes.clear();
        }
        compact
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

    pub const fn public(&self) -> &GamePublic {
        &self.public
    }

    pub const fn orientation(&self) -> Player {
        self.public.orientation
    }

    pub const fn outcome(&self) -> Outcome {
        self.public.outcome
    }

    pub const fn roster(&self) -> &Roster {
        &self.public.roster
    }

    pub const fn tags(&self) -> &[Tag] {
        self.public.tags.as_slice()
    }

    pub fn set_orientation(&mut self, orientation: Player) {
        self.update_public(|public| public.orientation = orientation);
    }

    pub fn set_outcome(&mut self, outcome: Outcome) {
        self.update_public(|public| public.outcome = outcome);
    }

    pub fn update_roster(&mut self, update: impl FnOnce(&mut Roster)) {
        self.update_public(|public| update(&mut public.roster));
    }

    pub fn update_tags(&mut self, update: impl FnOnce(&mut Vec<Tag>)) {
        self.update_public(|public| update(&mut public.tags));
    }

    pub fn update_public(&mut self, update: impl FnOnce(&mut GamePublic)) {
        let before = self.tracks_changes().then(|| self.public.clone());
        update(&mut self.public);
        if let Some(before) = before
            && before != self.public
        {
            self.track(change::Patch::Game(before));
        }
    }

    fn unchecked_with_mode(position: chess::Position, mode: Mode) -> Self {
        Self::unchecked_with_mode_next(position, mode, MoveId::MIN)
    }

    fn unchecked_with_mode_next(position: chess::Position, mode: Mode, next: MoveId) -> Self {
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

        Self { options: Vec::new(), position, check, legal, public: PositionPublic::default() }
    }

    pub const fn options(&self) -> &[MoveId] {
        self.options.as_slice()
    }

    pub const fn position(&self) -> chess::Position {
        self.position
    }

    pub fn comment(&self) -> Option<&Text> {
        self.public.comment.as_ref()
    }

    pub const fn valuation(&self) -> Option<Valuation> {
        self.public.valuation
    }

    pub const fn expanded(&self) -> bool {
        self.public.expanded
    }

    pub const fn legal(&self) -> &[chess::Move] {
        self.legal.as_slice()
    }

    pub const fn check(&self) -> Option<Check> {
        self.check
    }

    pub const fn is_checkmate(&self) -> bool {
        matches!(self.check, Some(Check::Checkmate))
    }

    pub const fn is_stalemate(&self) -> bool {
        self.check.is_none() && self.legal.is_empty()
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

    pub const fn commands(&self) -> &[Command] {
        self.public.commands.as_slice()
    }

    pub const fn nags(&self) -> &[Nag] {
        self.public.nags.as_slice()
    }

    pub const fn previous(&self) -> PositionId {
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

impl Command {
    pub fn expanded() -> Self {
        Self {
            command: Text::new("cgexp").expect("literal is non-empty"),
            parameters: vec!["true".to_string()],
        }
    }

    pub fn is_expanded(&self) -> bool {
        self.command.as_ref() == "cgexp"
            && self.parameters.len() == 1
            && self.parameters[0] == "true"
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

impl PositionId {
    pub const fn is_start(&self) -> bool {
        matches!(self, Self::Start)
    }
}

impl MoveId {
    pub const MIN: Self = Self(NonZeroU32::MIN);
    pub const MAX: Self = Self(NonZeroU32::MAX);

    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }

    pub const fn prev(self) -> Option<Self> {
        match NonZeroU32::new(self.0.get() - 1) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }

    /// Maps iterator.enumerate() indices 0, 1, 2...
    /// to Self::MIN = 1, Self::MIN + 1 = 2, etc.
    pub fn from_index(index: usize) -> Option<Self> {
        let id = u32::try_from(index).ok()?.checked_add(1)?;
        Some(Self(NonZeroU32::new(id)?))
    }
}

impl fmt::Display for MoveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for MoveId {
    type Err = <NonZeroU32 as FromStr>::Err;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        s.parse().map(Self)
    }
}

impl fmt::Display for PositionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", u32::from(*self))
    }
}

impl FromStr for PositionId {
    type Err = <u32 as FromStr>::Err;

    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        s.parse::<u32>().map(Self::from)
    }
}

impl From<MoveId> for PositionId {
    fn from(id: MoveId) -> Self {
        Self::Move(id)
    }
}

impl From<NonZeroU32> for MoveId {
    fn from(id: NonZeroU32) -> Self {
        Self(id)
    }
}

impl From<MoveId> for NonZeroU32 {
    fn from(id: MoveId) -> Self {
        id.0
    }
}

impl From<u32> for PositionId {
    fn from(id: u32) -> Self {
        match NonZeroU32::new(id) {
            Some(id) => Self::Move(MoveId(id)),
            None => Self::Start,
        }
    }
}

impl From<PositionId> for u32 {
    fn from(id: PositionId) -> Self {
        match id {
            PositionId::Move(id) => id.0.get(),
            PositionId::Start => 0,
        }
    }
}

impl Compact {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (MoveId, MoveId)> + '_ {
        self.0.iter().map(|(&before, &after)| (before, after))
    }

    pub fn play(&self, id: MoveId) -> Option<MoveId> {
        self.0.get(&id).copied()
    }

    pub fn position(&self, id: PositionId) -> Option<PositionId> {
        match id {
            PositionId::Move(id) => self.play(id).map(Into::into),
            PositionId::Start => Some(PositionId::Start),
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

    pub fn with_unremovable(&self, id: PositionId) -> Self {
        let mut protected = self.clone();
        protected.unremovable.insert(id);
        protected
    }
}

impl Score {
    pub const fn flip(self) -> Self {
        match self {
            Self::Centipawns(value) => Self::Centipawns(-value),
            Self::Mate(value) => Self::Mate(-value),
        }
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
        // Tree compaction derives the next move ID from the greatest position ID.
        assert!(PositionId::Start < PositionId::Move(MoveId::MIN));
    }

    #[test]
    fn position_ids_are_u32s() {
        for (position, integer) in [
            (PositionId::Start, 0),
            (PositionId::Move(MoveId::MIN), 1),
            (PositionId::Move(MoveId::MAX), u32::MAX),
        ] {
            assert_eq!(PositionId::from(integer), position);
            assert_eq!(u32::from(position), integer);
        }
    }

    #[test]
    fn move_ids_are_nonzero_u32s() {
        for integer in [NonZeroU32::MIN, NonZeroU32::MAX] {
            let id = MoveId::from(integer);
            assert_eq!(NonZeroU32::from(id), integer);
        }
    }

    #[test]
    fn ids_are_numeric_text() {
        let play = MoveId::from_index(21).unwrap();

        assert_eq!(play.to_string(), "22");
        assert_eq!("22".parse::<MoveId>().unwrap(), play);
        assert!("0".parse::<MoveId>().is_err());

        assert_eq!(PositionId::Start.to_string(), "0");
        assert_eq!("0".parse::<PositionId>().unwrap(), PositionId::Start);
        assert_eq!(PositionId::Move(play).to_string(), "22");
        assert_eq!("22".parse::<PositionId>().unwrap(), PositionId::Move(play));
    }

    #[test]
    fn ids_are_json_numbers() {
        let play = MoveId::from_index(21).unwrap();

        assert_eq!(serde_json::to_string(&play).unwrap(), "22");
        assert_eq!(serde_json::from_str::<MoveId>("22").unwrap(), play);

        assert_eq!(serde_json::to_string(&PositionId::Start).unwrap(), "0");
        assert_eq!(serde_json::from_str::<PositionId>("0").unwrap(), PositionId::Start);
        assert_eq!(serde_json::to_string(&PositionId::Move(play)).unwrap(), "22");
        assert_eq!(serde_json::from_str::<PositionId>("22").unwrap(), PositionId::Move(play));
    }
}
