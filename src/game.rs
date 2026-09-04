use std::{
    fmt,
    ops::{Deref, DerefMut},
};

use crate::{
    Move, Position,
    formats::san::Check,
    position::{Chess, Unvalidated},
    square::{File, Rank},
    variant::Variant,
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
pub struct Game<Variant = Chess> {
    pub roster: Roster,
    pub tags: Vec<Tag>,
    pub intro: Option<Text>,
    pub outcome: Outcome,
    /// State before any options are played.
    start: State<Variant>,
    tree: Tree<Variant>,
}

#[derive(Clone, PartialEq)]
pub struct Play<Variant = Chess> {
    slot: Slot,
    previous: Node,
    pub meta: Meta,
    /// The move played.
    play: Move,
    short: Short,
    /// State after playing this move, before any options are played.
    state: State<Variant>,
    /// Options to play after this position.
    options: Vec<Slot>,
}

#[derive(Clone, PartialEq)]
pub struct State<Variant = Chess> {
    position: Position<Variant>,
    legal: Vec<Move>,
    check: Option<Check>,
}

impl<V> Game<V> {
    pub fn start_options(&self) -> OptionsRef<'_, V> {
        self.options_ref(Node::Start)
    }

    pub fn play(&self, slot: Slot) -> Option<PlayRef<'_, V>> {
        self.tree.contains(slot).then_some(PlayRef { game: self, slot })
    }

    pub fn play_mut(&mut self, slot: Slot) -> Option<PlayMut<'_, V>> {
        self.tree.contains(slot).then_some(PlayMut { game: self, slot })
    }

    fn contains(&self, node: Node) -> bool {
        match node {
            Node::Start => true,
            Node::Play(slot) => self.tree.contains(slot),
        }
    }

    fn options_ref(&self, node: Node) -> OptionsRef<'_, V> {
        let options = self.tree.options(node);
        OptionsRef { game: self, node, options }
    }

    fn state(&self, node: Node) -> &State<V> {
        match node {
            Node::Start => &self.start,
            Node::Play(slot) => &self.tree.play(slot).expect("slot exists").state,
        }
    }

    fn delete_slot(&mut self, slot: Slot) {
        self.tree.remove(slot);
    }
}

impl<V> Game<V> {
    pub fn start(&self) -> Position<V> {
        self.start.position()
    }

    pub fn unvalidated(self) -> Game<Unvalidated> {
        Game {
            roster: self.roster,
            tags: self.tags,
            intro: self.intro,
            outcome: self.outcome,
            start: self.start.unvalidated(),
            tree: self.tree.unvalidated(),
        }
    }

    fn position(&self, node: Node) -> Position<V> {
        self.state(node).position()
    }
}

impl<V: Variant> Game<V> {
    pub fn new(position: Position<V>) -> Self {
        let legal = position.legal_moves();
        Self {
            roster: Default::default(),
            tags: Default::default(),
            intro: None,
            outcome: Default::default(),
            start: State { position, legal, check: None },
            tree: Tree::new(),
        }
    }

    pub fn start_options_mut(&mut self) -> OptionsMut<'_, V> {
        OptionsMut { game: self, node: Node::Start }
    }

    pub fn cursor(self) -> Cursor<V> {
        Cursor::new(self)
    }

    pub fn options(&self, node: Node) -> Option<OptionsRef<'_, V>> {
        if self.contains(node) { Some(self.options_ref(node)) } else { None }
    }

    pub fn options_mut(&mut self, node: Node) -> Option<OptionsMut<'_, V>> {
        if self.contains(node) { Some(OptionsMut { game: self, node }) } else { None }
    }

    // Responsible for validating the move, calculating derived state, assigning
    // a slot, and storing the play. It does not attach the play to the options of the node yet.
    fn create_play(&mut self, node: Node, play: Move) -> Result<Slot, Error> {
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

        let slot = self.tree.insert(|slot| Play {
            slot,
            previous: node,
            meta: Default::default(),
            state: State { position, legal, check },
            play,
            short,
            options: Default::default(),
        });

        Ok(slot)
    }
}

impl<V> Play<V> {
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

    pub fn legal(&self) -> &[Move] {
        self.state.legal()
    }
}

impl<V> Play<V> {
    pub fn position(&self) -> Position<V> {
        self.state.position()
    }

    pub fn unvalidated(self) -> Play<Unvalidated> {
        Play {
            slot: self.slot,
            previous: self.previous,
            meta: self.meta,
            play: self.play,
            short: self.short,
            state: self.state.unvalidated(),
            options: self.options,
        }
    }
}

impl<V> State<V> {
    #[inline]
    pub fn legal(&self) -> &[Move] {
        &self.legal
    }

    #[inline]
    pub fn check(&self) -> Option<Check> {
        self.check
    }
}

impl<V> State<V> {
    #[inline]
    pub fn position(&self) -> Position<V> {
        self.position
    }

    pub fn unvalidated(self) -> State<Unvalidated> {
        State { position: self.position.unvalidated(), legal: self.legal, check: self.check }
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

        if play.role == crate::Role::Pawn || play.is_castle() {
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Roster {
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub event: Option<Text>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub site: Option<Text>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub date: Option<Text>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub round: Option<Text>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub white: Option<Text>,
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub black: Option<Text>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Tag {
    pub key: Text,
    pub value: String,
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
    pub command: Text,
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

pub struct PlayRef<'a, Variant = Chess> {
    game: &'a Game<Variant>,
    slot: Slot,
}

impl<V> Deref for PlayRef<'_, V> {
    type Target = Play<V>;

    fn deref(&self) -> &Play<V> {
        self.game.tree.play(self.slot).expect("valid play")
    }
}

impl<'a, V> PlayRef<'a, V> {
    pub fn options(&self) -> OptionsRef<'a, V> {
        self.game.options_ref(Node::Play(self.slot))
    }
}

pub struct PlayMut<'a, Variant = Chess> {
    game: &'a mut Game<Variant>,
    slot: Slot,
}

impl<V> Deref for PlayMut<'_, V> {
    type Target = Play<V>;

    fn deref(&self) -> &Play<V> {
        self.game.tree.play(self.slot).expect("valid play")
    }
}

impl<V> DerefMut for PlayMut<'_, V> {
    fn deref_mut(&mut self) -> &mut Play<V> {
        self.game.tree.play_mut(self.slot).expect("valid play")
    }
}

impl<'a, V> PlayMut<'a, V> {
    pub fn options(&self) -> OptionsRef<'_, V> {
        self.game.options_ref(Node::Play(self.slot))
    }

    pub fn options_mut(&mut self) -> OptionsMut<'_, V> {
        OptionsMut { game: self.game, node: Node::Play(self.slot) }
    }

    pub fn into_options_mut(self) -> OptionsMut<'a, V> {
        OptionsMut { game: self.game, node: Node::Play(self.slot) }
    }
}

/// Read-only iterator over the options of a `Node`.
pub struct OptionsRef<'a, Variant = Chess> {
    game: &'a Game<Variant>,
    node: Node,
    options: &'a [Slot],
}

// Here and elsewhere, a #[derive(Clone, Copy)] won't work due to
// derive macro limitations - OptionsRef is in fact Copy
impl<V> Copy for OptionsRef<'_, V> {}

impl<V> Clone for OptionsRef<'_, V> {
    fn clone(&self) -> Self {
        *self
    }
}

// The Options Traversal API
impl<'a, V> OptionsRef<'a, V> {
    fn index(&self, play: Move) -> Option<usize> {
        self.iter().position(|option| option.play == play)
    }

    pub fn get(&self, play: Move) -> Option<PlayRef<'a, V>> {
        self.get_index(self.index(play)?)
    }

    pub fn contains(&self, play: Move) -> bool {
        self.index(play).is_some()
    }

    pub fn position(&self) -> Position<V> {
        self.game.position(self.node)
    }
    pub fn get_index(&self, index: usize) -> Option<PlayRef<'a, V>> {
        let slot = self.options.get(index)?;
        Some(PlayRef { game: self.game, slot: *slot })
    }

    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    pub fn len(&self) -> usize {
        self.options.len()
    }

    pub fn state(&self) -> &State<V> {
        self.game.state(self.node)
    }

    pub fn legal(&self) -> &[Move] {
        self.state().legal()
    }

    pub fn first(&self) -> Option<PlayRef<'a, V>> {
        self.get_index(0)
    }

    pub fn after_first(&self) -> OptionsRef<'a, V> {
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
    pub fn split_first(&self) -> Option<(PlayRef<'a, V>, OptionsRef<'a, V>)> {
        let (slot, rest) = self.options.split_first()?;
        Some((
            PlayRef { game: self.game, slot: *slot },
            OptionsRef { game: self.game, node: self.node, options: rest },
        ))
    }

    pub fn iter(self) -> OptionsIter<'a, V> {
        self.into_iter()
    }
}

impl<'a, V> IntoIterator for OptionsRef<'a, V> {
    type Item = PlayRef<'a, V>;
    type IntoIter = OptionsIter<'a, V>;

    fn into_iter(self) -> Self::IntoIter {
        OptionsIter { game: self.game, options: self.options, index: 0 }
    }
}

pub struct OptionsIter<'a, Variant = Chess> {
    game: &'a Game<Variant>,
    options: &'a [Slot],
    index: usize,
}

impl<'a, V> Iterator for OptionsIter<'a, V> {
    type Item = PlayRef<'a, V>;

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

impl<V> ExactSizeIterator for OptionsIter<'_, V> {
    fn len(&self) -> usize {
        self.options.len() - self.index
    }
}

pub struct OptionsMut<'a, Variant = Chess> {
    game: &'a mut Game<Variant>,
    node: Node,
}

// Forwarding to OptionsRef or "conversion of manipulation to traversal API"
// - explicit for now
// - can bring back helpful ones once the APIs settle
impl<V> OptionsMut<'_, V> {
    pub fn as_ref(&self) -> OptionsRef<'_, V> {
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
impl<'a, V: Variant> OptionsMut<'a, V> {
    pub fn push(&mut self, play: Move) -> Result<PlayMut<'_, V>, Error> {
        let slot = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).push(slot);
        Ok(PlayMut { game: self.game, slot })
    }

    pub fn into_push(self, play: Move) -> Result<PlayMut<'a, V>, Error> {
        let slot = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).push(slot);
        Ok(PlayMut { game: self.game, slot })
    }

    pub fn insert(&mut self, index: usize, play: Move) -> Result<PlayMut<'_, V>, Error> {
        let len = self.as_ref().len();
        if index > len {
            return Err(Error::OutOfBounds { index, len });
        }

        let slot = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).insert(index, slot);
        Ok(PlayMut { game: self.game, slot })
    }

    pub fn into_insert(self, index: usize, play: Move) -> Result<PlayMut<'a, V>, Error> {
        let len = self.as_ref().len();
        if index > len {
            return Err(Error::OutOfBounds { index, len });
        }

        let slot = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).insert(index, slot);
        Ok(PlayMut { game: self.game, slot })
    }

    pub fn into_get(self, play: Move) -> Option<PlayMut<'a, V>> {
        let index = self.as_ref().index(play)?;
        self.into_get_index(index)
    }

    pub fn into_get_index(self, index: usize) -> Option<PlayMut<'a, V>> {
        let slot = self.game.tree.options(self.node).get(index).copied()?;
        Some(PlayMut { game: self.game, slot })
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
