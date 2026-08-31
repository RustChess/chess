use std::{
    fmt,
    ops::{Deref, DerefMut},
};

use crate::{
    Move, Position,
    formats::san::Check,
    position::{File, Moves, Rank},
};

pub mod cursor;
pub use cursor::Cursor;
#[cfg(feature = "serde")]
pub mod storage;
pub mod tree;
pub use tree::{Node, Slot, Tree};

pub type Duplicate = usize;
pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("move already exists at index {0}")]
    Duplicate(Duplicate),
    #[error("illegal move")]
    Illegal,
    #[error("index {index} out of bounds for length {len}")]
    OutOfBounds { index: usize, len: usize },
}

/// A game of chess, including variations and annotations.
#[derive(Clone, PartialEq)]
pub struct Game {
    pub tags: Vec<TagPair>,
    pub intro: Option<Text>,
    pub outcome: Outcome,
    /// State before any options are played.
    start: State,
    tree: Tree,
}

#[derive(Clone, PartialEq)]
pub struct Play {
    slot: Slot,
    previous: Node,
    pub meta: Meta,
    /// The move played.
    play: Move,
    short: Short,
    /// State after playing this move, before any options are played.
    state: State,
    /// Options to play after this position.
    options: Vec<Slot>,
}

#[derive(Clone, PartialEq)]
pub struct State {
    position: Position,
    legal: Moves,
    check: Option<Check>,
}

impl Game {
    pub fn new(position: Position) -> Self {
        let legal = position.legal_moves();
        Self {
            tags: Default::default(),
            intro: None,
            outcome: Default::default(),
            start: State { position, legal, check: None },
            tree: Default::default(),
        }
    }

    pub fn start(&self) -> Position {
        self.start.position()
    }

    pub fn start_options(&self) -> OptionsRef<'_> {
        self.options_ref(Node::Start)
    }

    pub fn start_options_mut(&mut self) -> OptionsMut<'_> {
        OptionsMut { game: self, node: Node::Start }
    }

    pub fn play(&self, slot: Slot) -> Option<PlayRef<'_>> {
        self.tree.contains(slot).then_some(PlayRef { game: self, slot })
    }

    pub fn play_mut(&mut self, slot: Slot) -> Option<PlayMut<'_>> {
        self.tree.contains(slot).then_some(PlayMut { game: self, slot })
    }

    pub fn cursor(self) -> Cursor {
        Cursor::new(self)
    }

    pub fn options(&self, node: Node) -> Option<OptionsRef<'_>> {
        if self.contains(node) { Some(self.options_ref(node)) } else { None }
    }

    pub fn options_mut(&mut self, node: Node) -> Option<OptionsMut<'_>> {
        if self.contains(node) { Some(OptionsMut { game: self, node }) } else { None }
    }

    fn contains(&self, node: Node) -> bool {
        match node {
            Node::Start => true,
            Node::Play(slot) => self.tree.contains(slot),
        }
    }

    fn options_ref(&self, node: Node) -> OptionsRef<'_> {
        let options = self.tree.options(node);
        OptionsRef { game: self, node, options }
    }

    // Responsible for validating the move, calculating derived state, assigning
    // a slot, and storing the play. It does not attach the play to the options of the node yet.
    fn create_play(&mut self, node: Node, play: Move) -> Result<Play, Error> {
        // avoid move generation
        if let Some(index) = self.options_ref(node).index(play) {
            return Err(Error::Duplicate(index));
        }

        let state = self.state(node);
        if let Some(Check::Checkmate) = state.check() {
            return Err(Error::Illegal);
        }

        // legality check (legal moves are already computed)
        if !state.legal().contains(&play) {
            return Err(Error::Illegal);
        }

        // apply
        let position = state.position().apply_unchecked(play);

        // compute legal moves for the new position
        let legal = position.legal_moves();

        // update derived state
        let check = if position.is_check() {
            Some(if legal.is_empty() { Check::Checkmate } else { Check::Check })
        } else {
            None
        };
        let short = Short::new(state.legal(), play);

        let play = self.tree.insert(|slot| Play {
            slot,
            previous: node,
            meta: Default::default(),
            state: State { position, legal, check },
            play,
            short,
            options: Default::default(),
        });

        Ok(play)
    }

    fn position(&self, node: Node) -> Position {
        self.state(node).position()
    }

    fn state(&self, node: Node) -> &State {
        match node {
            Node::Start => &self.start,
            Node::Play(slot) => &self.tree.play(slot).expect("slot exists").state,
        }
    }

    fn delete_slot(&mut self, slot: Slot) {
        self.tree.remove(slot);
    }
}

impl Play {
    pub fn slot(&self) -> Slot {
        self.slot
    }

    pub fn previous(&self) -> Node {
        self.previous
    }

    pub fn play(&self) -> Move {
        self.play
    }

    pub fn short(&self) -> Short {
        self.short
    }

    pub fn check(&self) -> Option<Check> {
        self.state.check()
    }

    pub fn position(&self) -> Position {
        self.state.position()
    }

    pub fn legal(&self) -> &[Move] {
        self.state.legal()
    }
}

impl State {
    #[inline]
    pub fn position(&self) -> Position {
        self.position
    }

    #[inline]
    pub fn legal(&self) -> &[Move] {
        &self.legal
    }

    #[inline]
    pub fn check(&self) -> Option<Check> {
        self.check
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Meta {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub intro: Option<Text>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub comment: Option<Text>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub outro: Option<Text>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub nags: Vec<Nag>,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub commands: Vec<Command>,
}

impl Meta {
    pub fn is_empty(&self) -> bool {
        self.intro.is_none()
            && self.comment.is_none()
            && self.outro.is_none()
            && self.nags.is_empty()
            && self.commands.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Short {
    pub file: Option<File>,
    pub rank: Option<Rank>,
}

impl Short {
    pub fn new(legal: &[Move], play: Move) -> Self {
        let mut short = Short::default();

        if play.role == crate::position::Role::Pawn || play.is_castle() {
            return short;
        }

        let different_move = |other: &Move| *other != play;
        let same_role_and_to = |other: &Move| (other.role, other.to) == (play.role, play.to);
        let ambiguity = legal
            .iter()
            .copied()
            .filter(different_move)
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

#[derive(Default)]
struct Ambiguity {
    exists: bool,
    file: bool,
    rank: bool,
}

impl Ambiguity {
    fn consider(mut self, play: Move, other: Move) -> Self {
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

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct TagPair {
    pub name: Tag,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Tag {
    Event,
    Site,
    Date,
    Round,
    White,
    Black,
    Result,
    Fen,
    SetUp,
    Other(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Outcome {
    White,
    Black,
    Draw,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Text(String);

impl Text {
    pub fn new(text: impl AsRef<str>) -> Option<Self> {
        let text = text.as_ref().trim();
        (!text.is_empty()).then(|| Self(text.to_string()))
    }

    pub fn merge(&mut self, text: &Text) {
        self.0.push_str("\n\n");
        self.0.push_str(&text.0);
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for Text {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Command {
    pub command: String,
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Vec::is_empty"))]
    pub parameters: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(untagged))]
pub enum Nag {
    Numeric(u32),
    Symbol(String),
}

pub struct PlayRef<'a> {
    game: &'a Game,
    slot: Slot,
}

impl Deref for PlayRef<'_> {
    type Target = Play;

    fn deref(&self) -> &Play {
        self.game.tree.play(self.slot).expect("valid play")
    }
}

impl<'a> PlayRef<'a> {
    pub fn options(&self) -> OptionsRef<'a> {
        self.game.options_ref(Node::Play(self.slot))
    }
}

pub struct PlayMut<'a> {
    game: &'a mut Game,
    slot: Slot,
}

impl Deref for PlayMut<'_> {
    type Target = Play;

    fn deref(&self) -> &Play {
        self.game.tree.play(self.slot).expect("valid play")
    }
}

impl DerefMut for PlayMut<'_> {
    fn deref_mut(&mut self) -> &mut Play {
        self.game.tree.play_mut(self.slot).expect("valid play")
    }
}

impl PlayMut<'_> {
    pub fn options(&self) -> OptionsRef<'_> {
        self.game.options_ref(Node::Play(self.slot))
    }

    pub fn options_mut(&mut self) -> OptionsMut<'_> {
        OptionsMut { game: self.game, node: Node::Play(self.slot) }
    }
}

/// Read-only iterator over the options of a `Node`.
#[derive(Clone, Copy)]
pub struct OptionsRef<'a> {
    game: &'a Game,
    node: Node,
    options: &'a [Slot],
}

// The Options Traversal API
impl<'a> OptionsRef<'a> {
    fn get(&self, index: usize) -> Option<PlayRef<'a>> {
        let slot = self.options.get(index)?;
        Some(PlayRef { game: self.game, slot: *slot })
    }

    fn index(&self, play: Move) -> Option<usize> {
        self.iter().position(|option| option.play == play)
    }

    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    pub fn len(&self) -> usize {
        self.options.len()
    }

    pub fn contains(&self, play: Move) -> bool {
        self.index(play).is_some()
    }

    pub fn position(&self) -> Position {
        self.game.position(self.node)
    }

    pub fn state(&self) -> &State {
        self.game.state(self.node)
    }

    pub fn legal(&self) -> &[Move] {
        self.state().legal()
    }

    pub fn first(&self) -> Option<PlayRef<'a>> {
        self.get(0)
    }

    pub fn after_first(&self) -> OptionsRef<'a> {
        OptionsRef {
            game: self.game,
            node: self.node,
            options: self.options.get(1..).unwrap_or_default(),
        }
    }

    /// Combines [`Self::first`] and [`Self::after_first`].
    ///
    /// When this contains all options after a move, the first option is the
    /// mainline move, and the remaining options are variations from the same
    /// position.
    pub fn split_first(&self) -> Option<(PlayRef<'a>, OptionsRef<'a>)> {
        let (slot, rest) = self.options.split_first()?;
        Some((
            PlayRef { game: self.game, slot: *slot },
            OptionsRef { game: self.game, node: self.node, options: rest },
        ))
    }

    pub fn iter(self) -> OptionsIter<'a> {
        self.into_iter()
    }
}

impl<'a> IntoIterator for OptionsRef<'a> {
    type Item = PlayRef<'a>;
    type IntoIter = OptionsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        OptionsIter { game: self.game, options: self.options, index: 0 }
    }
}

pub struct OptionsIter<'a> {
    game: &'a Game,
    options: &'a [Slot],
    index: usize,
}

impl<'a> Iterator for OptionsIter<'a> {
    type Item = PlayRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let slot = self.options.get(self.index)?;
        self.index += 1;
        Some(PlayRef { game: self.game, slot: *slot })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl ExactSizeIterator for OptionsIter<'_> {
    fn len(&self) -> usize {
        self.options.len() - self.index
    }
}

pub struct OptionsMut<'a> {
    game: &'a mut Game,
    node: Node,
}

// Forwarding to OptionsRef or "conversion of manipulation to traversal API"
// - explicit for now
// - can bring back helpful ones once the APIs settle
impl OptionsMut<'_> {
    pub fn as_ref(&self) -> OptionsRef<'_> {
        self.game.options_ref(self.node)
    }

    // pub fn is_empty(&self) -> bool {
    //     self.as_ref().is_empty()
    // }

    // pub fn len(&self) -> usize {
    //     self.as_ref().len()
    // }

    // pub fn legal(&self) -> &[Move] {
    //     self.as_ref().state().legal()
    // }
}

// The Options Manipulation API
impl OptionsMut<'_> {
    pub fn push(&mut self, play: Move) -> Result<Slot, Error> {
        let play = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).push(play.slot);
        Ok(play.slot)
    }

    pub fn insert(&mut self, index: usize, play: Move) -> Result<Slot, Error> {
        let len = self.as_ref().len();
        if index > len {
            return Err(Error::OutOfBounds { index, len });
        }

        let play = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).insert(index, play.slot);
        Ok(play.slot)
    }

    #[must_use]
    pub fn remove(&mut self, play: Move) -> bool {
        let Some(index) = self.as_ref().index(play) else {
            return false;
        };
        self.remove_index(index)
    }

    fn remove_index(&mut self, index: usize) -> bool {
        let Some(slot) = self.game.tree.options_mut(self.node).get(index).copied() else {
            return false;
        };
        self.game.tree.options_mut(self.node).remove(index);
        self.game.delete_slot(slot);
        true
    }

    #[must_use]
    pub fn swap(&mut self, a: Move, b: Move) -> bool {
        let Some(a) = self.as_ref().index(a) else {
            return false;
        };
        let Some(b) = self.as_ref().index(b) else {
            return false;
        };
        self.swap_index(a, b)
    }

    fn swap_index(&mut self, a: usize, b: usize) -> bool {
        let len = self.as_ref().len();
        if a >= len || b >= len {
            false
        } else {
            self.game.tree.options_mut(self.node).swap(a, b);
            true
        }
    }

    #[must_use]
    pub fn raise(&mut self, play: Move) -> bool {
        let Some(index) = self.as_ref().index(play) else {
            return false;
        };
        self.raise_index(index)
    }

    fn raise_index(&mut self, index: usize) -> bool {
        if index >= self.as_ref().len() {
            false
        } else {
            index == 0 || self.swap_index(index - 1, index)
        }
    }

    #[must_use]
    pub fn lower(&mut self, play: Move) -> bool {
        let Some(index) = self.as_ref().index(play) else {
            return false;
        };
        self.lower_index(index)
    }

    fn lower_index(&mut self, index: usize) -> bool {
        let len = self.as_ref().len();
        if index >= len { false } else { index + 1 == len || self.swap_index(index, index + 1) }
    }

    #[must_use]
    pub fn promote(&mut self, play: Move) -> bool {
        let Some(index) = self.as_ref().index(play) else {
            return false;
        };
        self.promote_index(index)
    }

    fn promote_index(&mut self, index: usize) -> bool {
        let len = self.as_ref().len();
        if index >= len {
            return false;
        }

        let options = self.game.tree.options_mut(self.node);
        let option = options.remove(index);
        options.insert(0, option);
        true
    }

    // DANGEROUS, don't want such a thing.
    // For reads, can do self.as_ref().iter() and friends
    // pub fn for_each_mut(&mut self, mut f: impl FnMut(&mut Play)) {
    //     let ids = self.ids().to_vec();

    //     for slot in slots {
    //         f(self.game.slots.get_mut(&slot).expect("option slot must reference an existing play"));
    //     }
    // }
}
