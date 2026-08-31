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
    /// Position before any options are played.
    start: Position,
    cache: Cache,
    tree: Tree,
}

#[derive(Clone, PartialEq)]
pub struct Play {
    slot: Slot,
    previous: Node,
    pub meta: Meta,
    // Position after playing this move, before any options are played.
    position: Position,
    cache: Cache,
    play: Move,
    short: Short,
    /// Options to play after this position.
    options: Vec<Slot>,
}

#[derive(Clone, PartialEq)]
pub struct Cache {
    pub legal: Moves,
    pub check: Option<Check>,
}

impl Game {
    pub fn new(position: Position) -> Self {
        let legal = position.legal_moves();
        Self {
            tags: Default::default(),
            intro: None,
            outcome: Default::default(),
            start: position,
            cache: Cache { legal, check: None },
            tree: Default::default(),
        }
    }

    pub fn start(&self) -> Position {
        self.start
    }

    pub fn play(&self, slot: Slot) -> Option<PlayRef<'_>> {
        self.tree.contains(slot).then_some(PlayRef { game: self, slot })
    }

    pub fn play_mut(&mut self, slot: Slot) -> Option<PlayMut<'_>> {
        self.tree.contains(slot).then_some(PlayMut { game: self, slot })
    }

    pub fn options(&self) -> OptionsRef<'_> {
        self.options_ref(Node::Start)
    }

    pub fn options_mut(&mut self) -> OptionsMut<'_> {
        OptionsMut { game: self, node: Node::Start }
    }

    pub fn cursor(self) -> Cursor {
        Cursor::new(self)
    }

    pub fn options_mut_at(&mut self, node: Node) -> Option<OptionsMut<'_>> {
        if self.contains_node(node) { Some(OptionsMut { game: self, node }) } else { None }
    }

    fn contains_node(&self, node: Node) -> bool {
        match node {
            Node::Start => true,
            Node::Play(slot) => self.tree.contains(slot),
        }
    }

    fn options_ref(&self, node: Node) -> OptionsRef<'_> {
        let options = self.options_slice(node);
        OptionsRef { game: self, node, options }
    }

    fn options_slice(&self, node: Node) -> &[Slot] {
        self.tree.options(node)
    }

    fn options_mut_raw(&mut self, node: Node) -> &mut Vec<Slot> {
        self.tree.options_mut(node)
    }

    // push a new Move option in last position to the game's specified node.
    pub(crate) fn push_option(&mut self, node: Node, play: Move) -> Result<Slot, Error> {
        let play = self.create_play(node, play)?;
        self.options_mut_raw(node).push(play.slot);
        Ok(play.slot)
    }

    // insert a new Move option at the indicated position to the game's specified node.
    pub(crate) fn insert_option(
        &mut self,
        node: Node,
        index: usize,
        play: Move,
    ) -> Result<Slot, Error> {
        let len = self.options_ref(node).len();
        if index > len {
            return Err(Error::OutOfBounds { index, len });
        }
        let play = self.create_play(node, play)?;
        self.options_mut_raw(node).insert(index, play.slot);
        Ok(play.slot)
    }

    // Responsible for validating the move, calculating derived state, assigning
    // a slot, and storing the play. It does not attach the play to the options of the node yet.
    fn create_play(&mut self, node: Node, play: Move) -> Result<Play, Error> {
        // avoid move generation
        if let Some(index) = self.options_ref(node).index(play) {
            return Err(Error::Duplicate(index));
        }

        let previous_position = self.previous_position(node);
        let previous_cache = self.previous_cache(node);
        if let Some(Check::Checkmate) = previous_cache.check {
            return Err(Error::Illegal);
        }

        // legality check (legal moves are already computed)
        if !previous_cache.legal.contains(&play) {
            return Err(Error::Illegal);
        }

        // apply
        let position = previous_position.apply_unchecked(play);

        // compute legal moves - with cache this with the new position
        let legal = position.legal_moves();

        // update derived state
        let check = if position.is_check() {
            Some(if legal.is_empty() { Check::Checkmate } else { Check::Check })
        } else {
            None
        };
        let short = Short::new(&previous_cache.legal, play);

        let play = self.tree.insert(|slot| Play {
            slot,
            previous: node,
            meta: Default::default(),
            position,
            cache: Cache { legal, check },
            play,
            short,
            options: Default::default(),
        });

        Ok(play)
    }

    fn previous_position(&self, node: Node) -> Position {
        match node {
            Node::Start => self.start,
            Node::Play(slot) => self.tree.play(slot).expect("slot exists").position,
        }
    }

    fn previous_cache(&self, node: Node) -> &Cache {
        match node {
            Node::Start => &self.cache,
            Node::Play(slot) => &self.tree.play(slot).expect("slot exists").cache,
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
        self.cache.check
    }

    pub fn position(&self) -> Position {
        self.position
    }

    pub fn legal(&self) -> &[Move] {
        &self.cache.legal
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

#[derive(Clone)]
pub struct OptionsRef<'a> {
    game: &'a Game,
    #[allow(dead_code)]
    node: Node,
    options: &'a [Slot],
}

impl OptionsRef<'_> {
    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    pub fn len(&self) -> usize {
        self.options.len()
    }

    pub fn index(&self, play: Move) -> Option<usize> {
        self.values().position(|option| option.play == play)
    }

    pub fn contains(&self, play: Move) -> bool {
        self.index(play).is_some()
    }

    pub fn cache(&self) -> &Cache {
        self.game.previous_cache(self.node)
    }

    pub fn legal(&self) -> &[Move] {
        &self.cache().legal
    }

    pub fn options(&self) -> &[Slot] {
        self.options
    }

    pub fn get(&self, index: usize) -> Option<&Play> {
        let slot = self.options.get(index)?;
        Some(self.game.tree.play(*slot).expect("option must reference an existing play"))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Slot> {
        self.options().iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &Play> {
        self.iter().map(|(_, play)| play)
    }

    pub fn iter(&self) -> OptionsIter<'_> {
        OptionsIter { game: self.game, options: self.options, index: 0 }
    }
}

impl<'a> IntoIterator for OptionsRef<'a> {
    type Item = (&'a Slot, &'a Play);
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
    type Item = (&'a Slot, &'a Play);

    fn next(&mut self) -> Option<Self::Item> {
        let slot = self.options.get(self.index)?;
        self.index += 1;
        Some((slot, self.game.tree.play(*slot).expect("option must reference an existing play")))
    }
}

pub struct OptionsMut<'a> {
    game: &'a mut Game,
    node: Node,
}

impl OptionsMut<'_> {
    pub fn cache(&self) -> &Cache {
        self.game.previous_cache(self.node)
    }

    pub fn legal(&self) -> &[Move] {
        &self.cache().legal
    }

    pub fn as_ref(&self) -> OptionsRef<'_> {
        self.game.options_ref(self.node)
    }

    pub fn options(&self) -> &[Slot] {
        self.game.options_slice(self.node)
    }

    fn options_mut_raw(&mut self) -> &mut Vec<Slot> {
        self.game.options_mut_raw(self.node)
    }

    pub fn push(&mut self, play: Move) -> Result<Slot, Error> {
        self.game.push_option(self.node, play)
    }

    pub fn insert(&mut self, index: usize, play: Move) -> Result<Slot, Error> {
        self.game.insert_option(self.node, index, play)
    }

    #[must_use]
    pub fn remove(&mut self, play: Move) -> bool {
        let Some(index) = self.as_ref().index(play) else {
            return false;
        };
        self.remove_index(index)
    }

    fn remove_index(&mut self, index: usize) -> bool {
        let Some(slot) = self.options_mut_raw().get(index).copied() else {
            return false;
        };
        self.options_mut_raw().remove(index);
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
        let len = self.options().len();
        if a >= len || b >= len {
            false
        } else {
            self.options_mut_raw().swap(a, b);
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
        if index >= self.options().len() {
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
        let len = self.options().len();
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
        let len = self.options().len();
        if index >= len {
            return false;
        }

        let options = self.options_mut_raw();
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
