use core::ops::{Deref, DerefMut};

use crate::{
    Move, Player, Position,
    formats::{Text, san::Check},
    square::{File, Rank},
};

pub mod cursor;
pub use cursor::Cursor;
#[cfg(feature = "serde")]
pub mod storage;
pub mod tree;
pub use tree::{Node, Slot, Tree};

pub type Duplicate = usize;
pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Mode {
    #[default]
    Chess,
    Freestyle,
}

impl Mode {
    pub const fn is_freestyle(self) -> bool {
        matches!(self, Self::Freestyle)
    }
}

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
    pub roster: Roster,
    pub tags: Vec<Tag>,
    pub intro: Option<Text>,
    pub outcome: Outcome,
    pub orientation: Player,
    /// State before any options are played.
    start: State,
    tree: Tree,
    mode: Mode,
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
    options: Options,
}

/// Ordered options from a game position and their preferred presentation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Options {
    /// Whether variations in this option set are expanded.
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "is_false"))]
    pub expanded: bool,
    /// Ordered plays, with the mainline first.
    plays: Vec<Slot>,
}

#[cfg(feature = "serde")]
fn is_false(value: &bool) -> bool {
    !value
}

impl Options {
    pub const fn new() -> Self {
        Self { expanded: false, plays: Vec::new() }
    }
}

impl Deref for Options {
    type Target = Vec<Slot>;

    fn deref(&self) -> &Vec<Slot> {
        &self.plays
    }
}

impl DerefMut for Options {
    fn deref_mut(&mut self) -> &mut Vec<Slot> {
        &mut self.plays
    }
}

#[derive(Clone, PartialEq)]
pub struct State {
    pub evaluation: Option<Evaluation>,
    position: Position,
    legal: Vec<Move>,
    check: Option<Check>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
pub struct Evaluation {
    #[cfg_attr(feature = "serde", serde(default, skip_serializing_if = "Option::is_none"))]
    pub depth: Option<u32>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub score: Score,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Score {
    Centipawns(i32),
    Mate(i32),
}

impl Score {
    pub const fn flip(self) -> Self {
        match self {
            Self::Centipawns(value) => Self::Centipawns(-value),
            Self::Mate(value) => Self::Mate(-value),
        }
    }
}

impl Game {
    pub fn start_options(&self) -> OptionsRef<'_> {
        self.options_ref(Node::Start)
    }

    pub fn play(&self, slot: Slot) -> Option<PlayRef<'_>> {
        self.tree.contains(slot).then_some(PlayRef { game: self, slot })
    }

    pub fn play_mut(&mut self, slot: Slot) -> Option<PlayMut<'_>> {
        self.tree.contains(slot).then_some(PlayMut { game: self, slot })
    }

    fn contains(&self, node: Node) -> bool {
        match node {
            Node::Start => true,
            Node::Play(slot) => self.tree.contains(slot),
        }
    }

    fn options_ref(&self, node: Node) -> OptionsRef<'_> {
        let options = self.tree.options(node);
        OptionsRef { game: self, node, expanded: options.expanded, options: &options.plays }
    }

    fn state(&self, node: Node) -> &State {
        match node {
            Node::Start => &self.start,
            Node::Play(slot) => &self.tree.play(slot).expect("slot exists").state,
        }
    }

    fn state_mut(&mut self, node: Node) -> &mut State {
        match node {
            Node::Start => &mut self.start,
            Node::Play(slot) => &mut self.tree.play_mut(slot).expect("slot exists").state,
        }
    }

    fn delete_slot(&mut self, slot: Slot) {
        self.tree.remove(slot);
    }
}

impl Game {
    pub fn start(&self) -> Position {
        self.start.position()
    }

    pub const fn mode(&self) -> Mode {
        self.mode
    }

    fn position(&self, node: Node) -> Position {
        self.state(node).position()
    }
}

impl Game {
    pub fn new(position: Position, mode: Mode) -> Self {
        Self {
            roster: Default::default(),
            tags: Default::default(),
            intro: None,
            outcome: Default::default(),
            orientation: Player::White,
            start: State::new(position),
            tree: Tree::new(),
            mode,
        }
    }

    pub fn chess(position: Position) -> Option<Self> {
        position.castles().chess_compatible().then(|| Self::new(position, Mode::Chess))
    }

    pub fn freestyle(position: Position) -> Self {
        Self::new(position, Mode::Freestyle)
    }

    pub fn start_options_mut(&mut self) -> OptionsMut<'_> {
        OptionsMut { game: self, node: Node::Start }
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

        let short = Short::new(state.legal(), play);

        let slot = self.tree.insert(|slot| Play {
            slot,
            previous: node,
            meta: Default::default(),
            state: State::new(position),
            play,
            short,
            options: Default::default(),
        });

        Ok(slot)
    }
}

impl From<Position> for Game {
    fn from(position: Position) -> Self {
        let mode =
            if position.castles().chess_compatible() { Mode::Chess } else { Mode::Freestyle };
        Self::new(position, mode)
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

    pub fn legal(&self) -> &[Move] {
        self.state.legal()
    }

    pub fn state(&self) -> &State {
        &self.state
    }
}

impl Play {
    pub fn position(&self) -> Position {
        self.state.position()
    }
}

impl State {
    fn new(position: Position) -> Self {
        let legal = position.legal_moves();
        let check = if position.is_check() {
            Some(if legal.is_empty() { Check::Checkmate } else { Check::Check })
        } else {
            None
        };

        Self { evaluation: None, position, legal, check }
    }

    #[inline]
    pub fn legal(&self) -> &[Move] {
        &self.legal
    }

    #[inline]
    pub fn check(&self) -> Option<Check> {
        self.check
    }

    pub fn set_evaluation(&mut self, evaluation: Option<Evaluation>) -> bool {
        if self.evaluation == evaluation {
            false
        } else {
            self.evaluation = evaluation;
            true
        }
    }

    pub fn update_evaluation(&mut self, evaluation: Evaluation) -> bool {
        if self.evaluation.is_some_and(|previous| evaluation.depth < previous.depth) {
            false
        } else {
            self.set_evaluation(Some(evaluation))
        }
    }
}

impl State {
    #[inline]
    pub fn position(&self) -> Position {
        self.position
    }
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

pub struct PlayRef<'g> {
    game: &'g Game,
    slot: Slot,
}

impl Deref for PlayRef<'_> {
    type Target = Play;

    fn deref(&self) -> &Play {
        self.game.tree.play(self.slot).expect("valid play")
    }
}

impl<'g> PlayRef<'g> {
    pub fn options(&self) -> OptionsRef<'g> {
        self.game.options_ref(Node::Play(self.slot))
    }
}

pub struct PlayMut<'g> {
    game: &'g mut Game,
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

impl<'g> PlayMut<'g> {
    pub fn options(&self) -> OptionsRef<'_> {
        self.game.options_ref(Node::Play(self.slot))
    }

    pub fn options_mut(&mut self) -> OptionsMut<'_> {
        OptionsMut { game: self.game, node: Node::Play(self.slot) }
    }

    pub fn into_options_mut(self) -> OptionsMut<'g> {
        OptionsMut { game: self.game, node: Node::Play(self.slot) }
    }

    pub fn set_evaluation(&mut self, evaluation: Option<Evaluation>) -> bool {
        self.state.set_evaluation(evaluation)
    }

    pub fn update_evaluation(&mut self, evaluation: Evaluation) -> bool {
        self.state.update_evaluation(evaluation)
    }
}

/// Read-only iterator over the options of a `Node`.
pub struct OptionsRef<'g> {
    game: &'g Game,
    node: Node,
    expanded: bool,
    options: &'g [Slot],
}

// Here and elsewhere, a #[derive(Clone, Copy)] won't work due to
// derive macro limitations - OptionsRef is in fact Copy
impl Copy for OptionsRef<'_> {}

impl Clone for OptionsRef<'_> {
    fn clone(&self) -> Self {
        *self
    }
}

// The Options Traversal API
impl<'g> OptionsRef<'g> {
    fn index(&self, play: Move) -> Option<usize> {
        self.iter().position(|option| option.play == play)
    }

    pub fn get(&self, play: Move) -> Option<PlayRef<'g>> {
        self.get_index(self.index(play)?)
    }

    pub fn contains(&self, play: Move) -> bool {
        self.index(play).is_some()
    }

    pub const fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn position(&self) -> Position {
        self.game.position(self.node)
    }
    pub fn get_index(&self, index: usize) -> Option<PlayRef<'g>> {
        let slot = self.options.get(index)?;
        Some(PlayRef { game: self.game, slot: *slot })
    }

    pub fn is_empty(&self) -> bool {
        self.options.is_empty()
    }

    pub fn len(&self) -> usize {
        self.options.len()
    }

    pub const fn mode(&self) -> Mode {
        self.game.mode()
    }

    pub fn state(&self) -> &State {
        self.game.state(self.node)
    }

    pub fn legal(&self) -> &[Move] {
        self.state().legal()
    }

    pub fn first(&self) -> Option<PlayRef<'g>> {
        self.get_index(0)
    }

    pub fn after_first(&self) -> OptionsRef<'g> {
        OptionsRef {
            game: self.game,
            node: self.node,
            expanded: self.expanded,
            options: self.options.get(1..).unwrap_or_default(),
        }
    }

    /// Combines [`Self::first`] and [`Self::after_first`].
    ///
    /// When this contains all options after a move, the first option is the
    /// mainline move, and the remaining options are variations from the same
    /// position.
    pub fn split_first(&self) -> Option<(PlayRef<'g>, OptionsRef<'g>)> {
        let (slot, rest) = self.options.split_first()?;
        Some((
            PlayRef { game: self.game, slot: *slot },
            OptionsRef { game: self.game, node: self.node, expanded: self.expanded, options: rest },
        ))
    }

    pub fn iter(self) -> OptionsIter<'g> {
        self.into_iter()
    }
}

impl<'g> IntoIterator for OptionsRef<'g> {
    type Item = PlayRef<'g>;
    type IntoIter = OptionsIter<'g>;

    fn into_iter(self) -> Self::IntoIter {
        OptionsIter { game: self.game, options: self.options, index: 0 }
    }
}

pub struct OptionsIter<'g> {
    game: &'g Game,
    options: &'g [Slot],
    index: usize,
}

impl<'g> Iterator for OptionsIter<'g> {
    type Item = PlayRef<'g>;

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

pub struct OptionsMut<'g> {
    game: &'g mut Game,
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
impl<'g> OptionsMut<'g> {
    pub fn push(&mut self, play: Move) -> Result<PlayMut<'_>, Error> {
        let slot = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).push(slot);
        Ok(PlayMut { game: self.game, slot })
    }

    pub fn into_push(self, play: Move) -> Result<PlayMut<'g>, Error> {
        let slot = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).push(slot);
        Ok(PlayMut { game: self.game, slot })
    }

    pub fn insert(&mut self, index: usize, play: Move) -> Result<PlayMut<'_>, Error> {
        let len = self.as_ref().len();
        if index > len {
            return Err(Error::OutOfBounds { index, len });
        }

        let slot = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).insert(index, slot);
        Ok(PlayMut { game: self.game, slot })
    }

    pub fn into_insert(self, index: usize, play: Move) -> Result<PlayMut<'g>, Error> {
        let len = self.as_ref().len();
        if index > len {
            return Err(Error::OutOfBounds { index, len });
        }

        let slot = self.game.create_play(self.node, play)?;
        self.game.tree.options_mut(self.node).insert(index, slot);
        Ok(PlayMut { game: self.game, slot })
    }

    pub fn into_get(self, play: Move) -> Option<PlayMut<'g>> {
        let index = self.as_ref().index(play)?;
        self.into_get_index(index)
    }

    pub fn into_get_index(self, index: usize) -> Option<PlayMut<'g>> {
        let slot = self.game.tree.options(self.node).get(index).copied()?;
        Some(PlayMut { game: self.game, slot })
    }

    #[must_use]
    pub fn remove(&mut self, play: Move) -> Option<()> {
        let index = self.as_ref().index(play)?;
        self.remove_index(index)
    }

    fn remove_index(&mut self, index: usize) -> Option<()> {
        let slot = self.game.tree.options_mut(self.node).get(index).copied()?;
        self.game.tree.options_mut(self.node).remove(index);
        self.game.delete_slot(slot);
        Some(())
    }

    #[must_use]
    pub fn swap(&mut self, a: Move, b: Move) -> Option<bool> {
        let a = self.as_ref().index(a)?;
        let b = self.as_ref().index(b)?;
        self.swap_index(a, b)
    }

    fn swap_index(&mut self, a: usize, b: usize) -> Option<bool> {
        let len = self.as_ref().len();
        if a >= len || b >= len {
            None
        } else if a == b {
            Some(false)
        } else {
            self.game.tree.options_mut(self.node).swap(a, b);
            Some(true)
        }
    }

    #[must_use]
    pub fn raise(&mut self, play: Move) -> Option<bool> {
        let index = self.as_ref().index(play)?;
        self.raise_index(index)
    }

    fn raise_index(&mut self, index: usize) -> Option<bool> {
        if index >= self.as_ref().len() {
            None
        } else if index == 0 {
            Some(false)
        } else {
            self.swap_index(index - 1, index)
        }
    }

    #[must_use]
    pub fn lower(&mut self, play: Move) -> Option<bool> {
        let index = self.as_ref().index(play)?;
        self.lower_index(index)
    }

    fn lower_index(&mut self, index: usize) -> Option<bool> {
        let len = self.as_ref().len();
        if index >= len {
            None
        } else if index + 1 == len {
            Some(false)
        } else {
            self.swap_index(index, index + 1)
        }
    }

    #[must_use]
    pub fn promote(&mut self, play: Move) -> Option<bool> {
        let index = self.as_ref().index(play)?;
        self.promote_index(index)
    }

    #[must_use]
    pub fn demote(&mut self, play: Move) -> Option<bool> {
        let index = self.as_ref().index(play)?;
        let len = self.as_ref().len();
        if index >= len {
            return None;
        }
        if index + 1 == len {
            return Some(false);
        }

        let options = self.game.tree.options_mut(self.node);
        let option = options.remove(index);
        options.push(option);
        Some(true)
    }

    fn promote_index(&mut self, index: usize) -> Option<bool> {
        let len = self.as_ref().len();
        if index >= len {
            return None;
        }
        if index == 0 {
            return Some(false);
        }

        let options = self.game.tree.options_mut(self.node);
        let option = options.remove(index);
        options.insert(0, option);
        Some(true)
    }

    pub fn collapse(&mut self) {
        self.set_expanded(false);
    }

    pub fn expand(&mut self) {
        self.set_expanded(true);
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.game.tree.options_mut(self.node).expanded = expanded;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "serde")]
    #[test]
    fn serialize_evaluation_structurally() {
        let centipawns = Evaluation { depth: Some(18), score: Score::Centipawns(123) };
        let mate = Evaluation { depth: Some(22), score: Score::Mate(-3) };

        let centipawns_json = r#"{"depth":18,"centipawns":123}"#;
        let mate_json = r#"{"depth":22,"mate":-3}"#;
        assert_eq!(serde_json::to_string(&centipawns).unwrap(), centipawns_json);
        assert_eq!(serde_json::to_string(&mate).unwrap(), mate_json);
        assert_eq!(serde_json::from_str::<Evaluation>(centipawns_json).unwrap(), centipawns);
        assert_eq!(serde_json::from_str::<Evaluation>(mate_json).unwrap(), mate);
    }

    #[test]
    fn derive_check_for_start_position() {
        let check = Position::from_fen("4k3/8/8/8/8/8/8/K3R3 b - - 0 1").unwrap();
        let checkmate = Position::from_fen("k7/1Q6/2K5/8/8/8/8/8 b - - 0 1").unwrap();

        assert_eq!(Game::from(check).start_options().state().check(), Some(Check::Check));
        assert_eq!(Game::from(checkmate).start_options().state().check(), Some(Check::Checkmate));
    }
}
