use core::ops::Deref;

use std::collections::BTreeSet as Set;

use crate as chess;
use crate::{
    Player,
    formats::{Text, san::Check},
    square::{File, Rank},
};

// #[cfg(feature = "serde")]
// pub mod storage;

pub mod options;
pub mod patch;
pub mod traversal;

mod cursor;
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
    Patch(#[from] patch::Error),
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
    undo: patch::Undo,
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
#[serde(from = "Option<MoveId>", into = "Option<MoveId>")]
pub enum PositionId {
    Move(MoveId),
    #[default]
    Start,
}

/// The stable identity of a move within a game.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, PartialOrd, Eq, Ord, Serialize)]
#[serde(transparent)]
pub struct MoveId(u32);

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

    pub const fn mode(&self) -> Mode {
        self.mode
    }

    pub const fn has_undo(&self) -> bool {
        self.undo.on
    }

    pub fn set_undo(&mut self, on: bool) {
        self.undo.on = on;
        if !on {
            self.undo.stack.clear();
        }
    }

    pub fn patches(&self) -> &[patch::Patch] {
        &self.undo.stack
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

    fn update_public(&mut self, update: impl FnOnce(&mut GamePublic)) {
        let before = self.undo.on.then(|| self.public.clone());
        update(&mut self.public);
        if let Some(before) = before
            && before != self.public
        {
            self.undo.stack.push(patch::Patch::Game(before));
        }
    }

    fn unchecked_with_mode(position: chess::Position, mode: Mode) -> Self {
        Self::unchecked_with_mode_next(position, mode, MoveId::default())
    }

    fn unchecked_with_mode_next(position: chess::Position, mode: Mode, next: MoveId) -> Self {
        Self {
            mode,
            undo: Default::default(),
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
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }

    pub const fn prev(self) -> Option<Self> {
        match self.0.checked_sub(1) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }
}

impl From<MoveId> for PositionId {
    fn from(id: MoveId) -> Self {
        Self::Move(id)
    }
}

impl From<Option<MoveId>> for PositionId {
    fn from(id: Option<MoveId>) -> Self {
        match id {
            Some(id) => Self::Move(id),
            None => Self::Start,
        }
    }
}

impl From<PositionId> for Option<MoveId> {
    fn from(id: PositionId) -> Self {
        match id {
            PositionId::Move(id) => Some(id),
            PositionId::Start => None,
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
