use core::{
    borrow::{Borrow, BorrowMut},
    cmp::Ordering,
    iter::FusedIterator,
    ops::Deref,
    slice,
};

use std::collections::{BTreeMap as Map, BTreeSet as Set};

use crate::{
    Move as ChessPlay, Player, Position as ChessPosition,
    formats::{Text, san::Check},
};

use super::{Command, Evaluation, Mode, Nag, Outcome, Roster, Short, Tag};

/// A result of editing a game tree.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// An error from adding or locating an option.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("move already exists at index {0}")]
    Duplicate(usize),
    #[error("illegal move")]
    Illegal,
    #[error("option not found")]
    Missing,
}

/// An error indicating that removal would invalidate an attached value.
#[derive(Debug, thiserror::Error)]
#[error("position {0:?} may not be removed")]
pub struct Unremovable(pub PositionId);

/// A game tree and its game-level data.
#[derive(Clone, PartialEq)]
pub struct Game {
    pub roster: Roster,
    pub tags: Vec<Tag>,
    pub outcome: Outcome,
    pub orientation: Player,
    tree: Tree,
    mode: Mode,
}

/// Storage for the starting position and subsequent plays.
#[derive(Clone, PartialEq)]
struct Tree {
    next: PlayId,
    // PositionId::is_start() iff Node::play.is_none()
    nodes: Map<PositionId, Node>,
}

/// The stable identity of a play within a game.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct PlayId(u32);

/// The stable identity of a position within a game.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub enum PositionId {
    Play(PlayId),
    #[default]
    Start,
}

/// The starting position identity.
pub const START: PositionId = PositionId::Start;

/// The partial order of a position within a game tree.
#[derive(Clone, Copy)]
pub struct Order<'g> {
    game: &'g Game,
    pub id: PositionId,
}

impl From<PlayId> for PositionId {
    fn from(id: PlayId) -> Self {
        Self::Play(id)
    }
}

impl PositionId {
    pub const fn is_start(self) -> bool {
        matches!(self, Self::Start)
    }
}

impl PartialEq<PositionId> for Order<'_> {
    fn eq(&self, other: &PositionId) -> bool {
        self.id == *other
    }
}

impl PartialOrd<PositionId> for Order<'_> {
    fn partial_cmp(&self, other: &PositionId) -> Option<Ordering> {
        if self.id == *other {
            return Some(Ordering::Equal);
        }
        if !self.game.tree.contains(self.id) || !self.game.tree.contains(*other) {
            return None;
        }
        if self.game.tree.less_equal(self.id, *other) {
            Some(Ordering::Less)
        } else if self.game.tree.less_equal(*other, self.id) {
            Some(Ordering::Greater)
        } else {
            None
        }
    }
}

/// A position and the optional play that reached it.
#[derive(Clone, PartialEq)]
struct Node {
    play: Option<Play>,
    position: Position,
}

/// A move in a game, with its derived and public data.
#[derive(Clone, PartialEq)]
pub struct Play {
    // pointer left
    previous: PositionId,

    // core datum
    play: ChessPlay,

    // derived data
    short: Short,

    // public data
    pub affixes: Affixes,
    pub commands: Vec<Command>,
    pub nags: Vec<Nag>,
}

/// Comments surrounding an option in PGN movetext.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Affixes {
    pub intro: Option<Text>,
    pub outro: Option<Text>,
}

/// A position in a game, with its derived and public data.
#[derive(Clone, PartialEq)]
pub struct Position {
    // pointers right
    options: Vec<PlayId>,

    // core datum
    position: ChessPosition,

    // derived data
    check: Option<Check>,
    legal: Vec<ChessPlay>,

    // public data
    pub comment: Option<Text>,
    pub evaluation: Option<Evaluation>,
    pub expanded: bool,
}

/// An immutable reference to a play in a game.
#[derive(Clone, Copy)]
pub struct PlayRef<'g> {
    game: &'g Game,
    id: PlayId,
}

/// A mutable reference to a play in a game.
pub struct PlayMut<'g> {
    game: &'g mut Game,
    id: PlayId,
    protected: Protected,
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

/// Positions retained by attached mutable values.
#[derive(Clone)]
struct Protected {
    unremovable: Set<PositionId>,
}

/// Locates an option within a game position.
pub trait Locate {
    fn locate(self, position: PositionRef<'_>) -> Option<usize>;
}

/// Iterator over the plays from a position.
#[derive(Clone)]
pub struct Plays<'g> {
    game: &'g Game,
    ids: slice::Iter<'g, PlayId>,
}

/// Lending iterator over the options from a position in index order.
///
/// Options removed before being reached are skipped.
pub struct PlaysMut<'g> {
    game: &'g mut Game,
    position: PositionId,
    index: usize,
    visited: Set<PlayId>,
    protected: Protected,
}

impl Protected {
    const fn new() -> Self {
        Self { unremovable: Set::new() }
    }

    fn with_unremovable(&self, id: PositionId) -> Self {
        let mut protected = self.clone();
        protected.unremovable.insert(id);
        protected
    }
}

impl<'g> PlayMut<'g> {
    const fn new(game: &'g mut Game, id: PlayId) -> Self {
        Self { game, id, protected: Protected::new() }
    }
}

impl<'g> PositionMut<'g> {
    const fn new(game: &'g mut Game, id: PositionId) -> Self {
        Self { game, id, protected: Protected::new() }
    }
}

impl Play {
    pub const fn previous(&self) -> PositionId {
        self.previous
    }

    pub const fn play(&self) -> ChessPlay {
        self.play
    }

    pub const fn short(&self) -> Short {
        self.short
    }
}

impl Position {
    fn new(position: ChessPosition) -> Self {
        let legal = position.legal_moves();
        let check = Check::new(position.is_check(), legal.is_empty());

        Self {
            options: Vec::new(),
            position,
            check,
            legal,
            comment: None,
            evaluation: None,
            expanded: false,
        }
    }

    pub const fn options(&self) -> &[PlayId] {
        self.options.as_slice()
    }

    pub const fn position(&self) -> ChessPosition {
        self.position
    }

    pub const fn legal(&self) -> &[ChessPlay] {
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

impl Node {
    const fn play(&self) -> &Play {
        self.play.as_ref().expect("play exists")
    }

    const fn play_mut(&mut self) -> &mut Play {
        self.play.as_mut().expect("play exists")
    }

    const fn position(&self) -> &Position {
        &self.position
    }

    const fn position_mut(&mut self) -> &mut Position {
        &mut self.position
    }
}

impl Deref for Play {
    type Target = ChessPlay;

    fn deref(&self) -> &ChessPlay {
        &self.play
    }
}

impl Deref for Position {
    type Target = ChessPosition;

    fn deref(&self) -> &ChessPosition {
        &self.position
    }
}

impl Tree {
    fn contains(&self, id: PositionId) -> bool {
        self.nodes.contains_key(&id)
    }

    fn less_equal(&self, ancestor: PositionId, mut child: PositionId) -> bool {
        while ancestor != child {
            let PositionId::Play(id) = child else { return false };
            child = self.play(id).previous;
        }
        true
    }

    fn node(&self, id: PositionId) -> &Node {
        self.nodes.get(&id).expect("position exists")
    }

    fn node_mut(&mut self, id: PositionId) -> &mut Node {
        self.nodes.get_mut(&id).expect("position exists")
    }

    fn play(&self, id: PlayId) -> &Play {
        self.node(id.into()).play()
    }

    fn play_mut(&mut self, id: PlayId) -> &mut Play {
        self.node_mut(id.into()).play_mut()
    }

    fn position(&self, id: PositionId) -> &Position {
        self.node(id).position()
    }

    fn position_mut(&mut self, id: PositionId) -> &mut Position {
        self.node_mut(id).position_mut()
    }

    fn insert(&mut self, node: Node) -> PlayId {
        let id = self.next;
        self.next.0 += 1;
        self.nodes.insert(id.into(), node);
        id
    }

    fn remove(&mut self, id: PlayId) -> bool {
        let Some(node) = self.nodes.remove(&id.into()) else { return false };
        for option in node.position.options {
            self.remove(option);
        }
        true
    }
}

impl Game {
    pub fn play(&self, id: PlayId) -> Option<PlayRef<'_>> {
        self.tree.contains(id.into()).then_some(PlayRef { game: self, id })
    }

    pub fn play_mut(&mut self, id: PlayId) -> Option<PlayMut<'_>> {
        self.tree.contains(id.into()).then_some(PlayMut::new(self, id))
    }

    pub fn position(&self, id: PositionId) -> Option<PositionRef<'_>> {
        self.tree.contains(id).then_some(PositionRef { game: self, id })
    }

    pub fn position_mut(&mut self, id: PositionId) -> Option<PositionMut<'_>> {
        self.tree.contains(id).then_some(PositionMut::new(self, id))
    }

    pub const fn start(&self) -> PositionRef<'_> {
        PositionRef { game: self, id: START }
    }

    pub const fn start_mut(&mut self) -> PositionMut<'_> {
        PositionMut::new(self, START)
    }

    pub const fn cursor(&self) -> CursorRef<'_> {
        Cursor { game: self, current: START, preserve: Set::new() }
    }

    pub const fn cursor_mut(&mut self) -> CursorMut<'_> {
        Cursor { game: self, current: START, preserve: Set::new() }
    }

    pub const fn order(&self, id: PositionId) -> Order<'_> {
        Order { game: self, id }
    }
}

/// Access API.
impl<'g> PlayRef<'g> {
    pub const fn id(&self) -> PlayId {
        self.id
    }

    pub const fn game(&self) -> &'g Game {
        self.game
    }

    fn play(&self) -> &Play {
        self.game.tree.play(self.id)
    }
}

/// Traversal API.
impl<'g> PlayRef<'g> {
    pub fn previous(&self) -> PositionRef<'g> {
        PositionRef { game: self.game, id: self.play().previous }
    }

    pub const fn position(&self) -> PositionRef<'g> {
        PositionRef { game: self.game, id: PositionId::Play(self.id) }
    }
}

impl Deref for PlayRef<'_> {
    type Target = Play;

    fn deref(&self) -> &Play {
        self.play()
    }
}

/// Access API.
impl<'g> PlayMut<'g> {
    pub const fn id(&self) -> PlayId {
        self.id
    }

    pub const fn game(&self) -> &Game {
        self.game
    }

    pub const fn as_ref(&self) -> PlayRef<'_> {
        PlayRef { game: self.game, id: self.id }
    }

    pub fn into_ref(self) -> PlayRef<'g> {
        PlayRef { game: self.game, id: self.id }
    }

    pub fn affixes_mut(&mut self) -> &mut Affixes {
        &mut self.play_mut().affixes
    }

    pub fn into_affixes_mut(self) -> &'g mut Affixes {
        &mut self.game.tree.play_mut(self.id).affixes
    }

    pub fn commands_mut(&mut self) -> &mut Vec<Command> {
        &mut self.play_mut().commands
    }

    pub fn into_commands_mut(self) -> &'g mut Vec<Command> {
        &mut self.game.tree.play_mut(self.id).commands
    }

    pub fn nags_mut(&mut self) -> &mut Vec<Nag> {
        &mut self.play_mut().nags
    }

    pub fn into_nags_mut(self) -> &'g mut Vec<Nag> {
        &mut self.game.tree.play_mut(self.id).nags
    }

    fn play(&self) -> &Play {
        self.game.tree.play(self.id)
    }

    fn play_mut(&mut self) -> &mut Play {
        self.game.tree.play_mut(self.id)
    }
}

/// Traversal API.
impl<'g> PlayMut<'g> {
    pub fn previous(&self) -> PositionRef<'_> {
        self.as_ref().previous()
    }

    pub fn previous_mut(&mut self) -> PositionMut<'_> {
        let id = self.play().previous;
        let protected = self.protected.with_unremovable(self.id.into());
        PositionMut { game: self.game, id, protected }
    }

    pub fn into_previous(self) -> PositionMut<'g> {
        let id = self.play().previous;
        PositionMut { game: self.game, id, protected: self.protected }
    }

    pub const fn position(&self) -> PositionRef<'_> {
        PositionRef { game: self.game, id: PositionId::Play(self.id) }
    }

    pub fn position_mut(&mut self) -> PositionMut<'_> {
        PositionMut {
            game: self.game,
            id: PositionId::Play(self.id),
            protected: self.protected.clone(),
        }
    }

    pub fn into_position(self) -> PositionMut<'g> {
        PositionMut { game: self.game, id: PositionId::Play(self.id), protected: self.protected }
    }
}

impl Deref for PlayMut<'_> {
    type Target = Play;

    fn deref(&self) -> &Play {
        self.play()
    }
}

/// Access API.
impl<'g> PositionRef<'g> {
    pub const fn id(&self) -> PositionId {
        self.id
    }

    pub const fn game(&self) -> &'g Game {
        self.game
    }

    pub const fn cursor(self) -> CursorRef<'g> {
        Cursor { game: self.game, current: self.id, preserve: Set::new() }
    }

    pub fn index(&self, option: impl Locate) -> Option<usize> {
        let index = option.locate(*self)?;
        (index < self.options().len()).then_some(index)
    }

    pub fn contains(&self, option: impl Locate) -> bool {
        self.index(option).is_some()
    }

    fn position(&self) -> &'g Position {
        self.game.tree.position(self.id)
    }
}

/// Traversal API.
impl<'g> PositionRef<'g> {
    pub const fn play(&self) -> Option<PlayRef<'g>> {
        match self.id {
            PositionId::Play(id) => Some(PlayRef { game: self.game, id }),
            PositionId::Start => None,
        }
    }

    pub fn get(&self, option: impl Locate) -> Option<PlayRef<'g>> {
        let index = self.index(option)?;
        let id = self.position().options.get(index).copied()?;
        Some(PlayRef { game: self.game, id })
    }

    pub fn main(&self) -> Option<PlayRef<'g>> {
        self.get(0)
    }

    pub fn has_alternatives(&self) -> bool {
        self.position().options.len() >= 2
    }

    pub fn alternatives(&self) -> Plays<'g> {
        let ids = self.position().options.get(1..).unwrap_or_default().iter();
        Plays { game: self.game, ids }
    }

    pub fn iter(&self) -> Plays<'g> {
        Plays { game: self.game, ids: self.position().options.iter() }
    }
}

impl Deref for PositionRef<'_> {
    type Target = Position;

    fn deref(&self) -> &Position {
        self.position()
    }
}

impl Locate for usize {
    fn locate(self, position: PositionRef<'_>) -> Option<usize> {
        (self < position.options().len()).then_some(self)
    }
}

impl Locate for PlayId {
    fn locate(self, position: PositionRef<'_>) -> Option<usize> {
        position.options().iter().position(|id| *id == self)
    }
}

impl Locate for ChessPlay {
    fn locate(self, position: PositionRef<'_>) -> Option<usize> {
        position.iter().position(|option| **option == self)
    }
}

impl<'g> IntoIterator for PositionRef<'g> {
    type Item = PlayRef<'g>;
    type IntoIter = Plays<'g>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'g> Iterator for Plays<'g> {
    type Item = PlayRef<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.ids.next().copied()?;
        Some(PlayRef { game: self.game, id })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ids.size_hint()
    }
}

impl DoubleEndedIterator for Plays<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let id = self.ids.next_back().copied()?;
        Some(PlayRef { game: self.game, id })
    }
}

impl ExactSizeIterator for Plays<'_> {}
impl FusedIterator for Plays<'_> {}

impl PlaysMut<'_> {
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<PlayMut<'_>> {
        loop {
            let id = self.game.tree.position(self.position).options.get(self.index).copied()?;
            self.index += 1;

            if self.visited.insert(id) && self.game.tree.contains(id.into()) {
                return Some(PlayMut { game: self.game, id, protected: self.protected.clone() });
            }
        }
    }
}

/// Access API.
impl<'g> PositionMut<'g> {
    pub const fn id(&self) -> PositionId {
        self.id
    }

    pub const fn game(&self) -> &Game {
        self.game
    }

    pub const fn as_ref(&self) -> PositionRef<'_> {
        PositionRef { game: self.game, id: self.id }
    }

    pub fn into_ref(self) -> PositionRef<'g> {
        PositionRef { game: self.game, id: self.id }
    }

    pub fn cursor_mut(&mut self) -> CursorMut<'_> {
        let mut preserve = self.protected.unremovable.clone();
        preserve.insert(self.id);
        Cursor { game: self.game, current: self.id, preserve }
    }

    pub fn into_cursor_mut(self) -> CursorMut<'g> {
        Cursor { game: self.game, current: self.id, preserve: self.protected.unremovable }
    }

    pub fn comment_mut(&mut self) -> &mut Option<Text> {
        &mut self.position_mut().comment
    }

    pub fn into_comment_mut(self) -> &'g mut Option<Text> {
        &mut self.game.tree.position_mut(self.id).comment
    }

    pub fn evaluation_mut(&mut self) -> &mut Option<Evaluation> {
        &mut self.position_mut().evaluation
    }

    pub fn into_evaluation_mut(self) -> &'g mut Option<Evaluation> {
        &mut self.game.tree.position_mut(self.id).evaluation
    }

    pub fn set_evaluation(&mut self, evaluation: Option<Evaluation>) -> bool {
        if self.evaluation != evaluation {
            *self.evaluation_mut() = evaluation;
            true
        } else {
            false
        }
    }

    // Only set if we have at least same depth evaluation.
    // For Multi-PV lines, they are not stable for a given depth
    // until all lines at that depth were determined, hence `<`, not `<=`
    pub fn update_evaluation(&mut self, evaluation: Evaluation) -> bool {
        if self.evaluation.is_some_and(|previous| evaluation.depth < previous.depth) {
            false
        } else {
            self.set_evaluation(Some(evaluation))
        }
    }

    pub fn expanded_mut(&mut self) -> &mut bool {
        &mut self.position_mut().expanded
    }

    pub fn into_expanded_mut(self) -> &'g mut bool {
        &mut self.game.tree.position_mut(self.id).expanded
    }

    fn position(&self) -> &Position {
        self.game.tree.position(self.id)
    }

    fn position_mut(&mut self) -> &mut Position {
        self.game.tree.position_mut(self.id)
    }
}

/// Traversal API.
impl<'g> PositionMut<'g> {
    pub const fn play(&self) -> Option<PlayRef<'_>> {
        match self.id {
            PositionId::Play(id) => Some(PlayRef { game: self.game, id }),
            PositionId::Start => None,
        }
    }

    pub fn play_mut(&mut self) -> Option<PlayMut<'_>> {
        match self.id {
            PositionId::Play(id) => {
                Some(PlayMut { game: self.game, id, protected: self.protected.clone() })
            }
            PositionId::Start => None,
        }
    }

    pub fn into_play(self) -> Option<PlayMut<'g>> {
        match self.id {
            PositionId::Play(id) => {
                Some(PlayMut { game: self.game, id, protected: self.protected })
            }
            PositionId::Start => None,
        }
    }

    pub fn get_mut(&mut self, option: impl Locate) -> Option<PlayMut<'_>> {
        let id = self.as_ref().get(option)?.id();
        Some(PlayMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_get(self, option: impl Locate) -> Option<PlayMut<'g>> {
        let id = self.as_ref().get(option)?.id();
        Some(PlayMut { game: self.game, id, protected: self.protected })
    }

    pub fn main_mut(&mut self) -> Option<PlayMut<'_>> {
        self.get_mut(0)
    }

    pub fn into_main(self) -> Option<PlayMut<'g>> {
        self.into_get(0)
    }

    pub fn alternatives_mut(&mut self) -> PlaysMut<'_> {
        PlaysMut {
            game: self.game,
            position: self.id,
            index: 1,
            visited: Set::new(),
            protected: self.protected.clone(),
        }
    }

    pub fn into_alternatives(self) -> PlaysMut<'g> {
        PlaysMut {
            game: self.game,
            position: self.id,
            index: 1,
            visited: Set::new(),
            protected: self.protected,
        }
    }

    pub fn iter_mut(&mut self) -> PlaysMut<'_> {
        PlaysMut {
            game: self.game,
            position: self.id,
            index: 0,
            visited: Set::new(),
            protected: self.protected.clone(),
        }
    }

    pub fn into_iter_mut(self) -> PlaysMut<'g> {
        PlaysMut {
            game: self.game,
            position: self.id,
            index: 0,
            visited: Set::new(),
            protected: self.protected,
        }
    }
}

/// Options API.
impl<'g> PositionMut<'g> {
    pub fn push(&mut self, play: ChessPlay) -> Result<()> {
        self.insert_at(self.options().len(), play).map(drop)
    }

    pub fn push_mut(&mut self, play: ChessPlay) -> Result<PlayMut<'_>> {
        let id = self.insert_at(self.options().len(), play)?;
        Ok(PlayMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_push(mut self, play: ChessPlay) -> Result<PlayMut<'g>> {
        let id = self.insert_at(self.options().len(), play)?;
        Ok(PlayMut { game: self.game, id, protected: self.protected })
    }

    pub fn insert_before(&mut self, option: impl Locate, play: ChessPlay) -> Result<()> {
        self.insert_before_option(option, play).map(drop)
    }

    pub fn insert_before_mut(
        &mut self,
        option: impl Locate,
        play: ChessPlay,
    ) -> Result<PlayMut<'_>> {
        let id = self.insert_before_option(option, play)?;
        Ok(PlayMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_insert_before(
        mut self,
        option: impl Locate,
        play: ChessPlay,
    ) -> Result<PlayMut<'g>> {
        let id = self.insert_before_option(option, play)?;
        Ok(PlayMut { game: self.game, id, protected: self.protected })
    }

    pub fn insert_after(&mut self, option: impl Locate, play: ChessPlay) -> Result<()> {
        self.insert_after_option(option, play).map(drop)
    }

    pub fn insert_after_mut(
        &mut self,
        option: impl Locate,
        play: ChessPlay,
    ) -> Result<PlayMut<'_>> {
        let id = self.insert_after_option(option, play)?;
        Ok(PlayMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_insert_after(
        mut self,
        option: impl Locate,
        play: ChessPlay,
    ) -> Result<PlayMut<'g>> {
        let id = self.insert_after_option(option, play)?;
        Ok(PlayMut { game: self.game, id, protected: self.protected })
    }

    pub fn pop(&mut self) -> core::result::Result<Option<()>, Unremovable> {
        let Some(index) = self.options().len().checked_sub(1) else { return Ok(None) };
        self.remove_index(index)
    }

    pub fn remove(&mut self, option: impl Locate) -> core::result::Result<Option<()>, Unremovable> {
        let Some(index) = self.as_ref().index(option) else { return Ok(None) };
        self.remove_index(index)
    }

    pub fn swap(&mut self, a: impl Locate, b: impl Locate) -> Option<bool> {
        let position = self.as_ref();
        let a = position.index(a)?;
        let b = position.index(b)?;
        Some(self.swap_index(a, b))
    }

    pub fn raise(&mut self, option: impl Locate) -> Option<bool> {
        let index = self.as_ref().index(option)?;
        if index == 0 {
            Some(false)
        } else {
            self.swap_index(index - 1, index);
            Some(true)
        }
    }

    pub fn lower(&mut self, option: impl Locate) -> Option<bool> {
        let index = self.as_ref().index(option)?;
        if index + 1 == self.options().len() {
            Some(false)
        } else {
            self.swap_index(index, index + 1);
            Some(true)
        }
    }

    pub fn promote(&mut self, option: impl Locate) -> Option<bool> {
        let index = self.as_ref().index(option)?;
        if index == 0 {
            return Some(false);
        }

        let id = self.position_mut().options.remove(index);
        self.position_mut().options.insert(0, id);
        Some(true)
    }

    pub fn demote(&mut self, option: impl Locate) -> Option<bool> {
        let index = self.as_ref().index(option)?;
        if index + 1 == self.options().len() {
            return Some(false);
        }

        let id = self.position_mut().options.remove(index);
        self.position_mut().options.push(id);
        Some(true)
    }

    fn insert_before_option(&mut self, option: impl Locate, play: ChessPlay) -> Result<PlayId> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        self.insert_at(index, play)
    }

    fn insert_after_option(&mut self, option: impl Locate, play: ChessPlay) -> Result<PlayId> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        self.insert_at(index + 1, play)
    }

    fn insert_at(&mut self, index: usize, play: ChessPlay) -> Result<PlayId> {
        let node = {
            let position = self.as_ref();
            if let Some(index) = position.index(play) {
                return Err(Error::Duplicate(index));
            }
            if !position.legal().contains(&play) {
                return Err(Error::Illegal);
            }

            let next = position.position().apply_unchecked(play);
            let play = Play {
                previous: self.id,
                play,
                short: Short::new(position.legal(), play),
                affixes: Affixes::default(),
                commands: Vec::new(),
                nags: Vec::new(),
            };
            Node { play: Some(play), position: Position::new(next) }
        };

        let id = self.game.tree.insert(node);
        self.position_mut().options.insert(index, id);
        Ok(id)
    }

    fn remove_index(&mut self, index: usize) -> core::result::Result<Option<()>, Unremovable> {
        let Some(id) = self.position().options.get(index).copied() else { return Ok(None) };
        if let Some(id) =
            self.protected.unremovable.iter().copied().find(|at| self.game.order(*at) >= id.into())
        {
            return Err(Unremovable(id));
        }

        self.position_mut().options.remove(index);
        let removed = self.game.tree.remove(id);
        debug_assert!(removed);
        Ok(Some(()))
    }

    fn swap_index(&mut self, a: usize, b: usize) -> bool {
        if a == b {
            false
        } else {
            self.position_mut().options.swap(a, b);
            true
        }
    }
}

impl Deref for PositionMut<'_> {
    type Target = Position;

    fn deref(&self) -> &Position {
        self.position()
    }
}

impl Tree {
    fn new(position: ChessPosition) -> Self {
        let start = Node { play: None, position: Position::new(position) };
        Self { next: PlayId::default(), nodes: Map::from([(START, start)]) }
    }
}

impl Game {
    pub fn new(position: ChessPosition) -> Self {
        let mode = position.requires_freestyle().into();
        Self::with_mode(position, mode)
    }

    pub fn chess(position: ChessPosition) -> Option<Self> {
        (!position.requires_freestyle()).then(|| Self::with_mode(position, Mode::Chess))
    }

    pub fn freestyle(position: ChessPosition) -> Self {
        Self::with_mode(position, Mode::Freestyle)
    }

    pub const fn mode(&self) -> Mode {
        self.mode
    }

    fn with_mode(position: ChessPosition, mode: Mode) -> Self {
        Self {
            roster: Roster::default(),
            tags: Vec::new(),
            outcome: Outcome::default(),
            orientation: Player::White,
            tree: Tree::new(position),
            mode,
        }
    }
}

/// A movable position within a game tree.
#[derive(Clone, PartialEq)]
pub struct Cursor<G = Game> {
    game: G,
    current: PositionId,
    preserve: Set<PositionId>,
}

/// A cursor over an immutably borrowed game.
pub type CursorRef<'g> = Cursor<&'g Game>;
/// A cursor over a mutably borrowed game.
pub type CursorMut<'g> = Cursor<&'g mut Game>;

impl Cursor<Game> {
    pub const fn new(game: Game) -> Self {
        Self { game, current: START, preserve: Set::new() }
    }

    pub fn into_game(self) -> Game {
        self.game
    }
}

/// Access API.
impl<G> Cursor<G> {
    pub const fn current(&self) -> PositionId {
        self.current
    }
}

/// Access API.
impl<G: Borrow<Game>> Cursor<G> {
    pub fn game(&self) -> &Game {
        self.game.borrow()
    }

    pub fn position(&self) -> PositionRef<'_> {
        self.game().position(self.current).expect("cursor position exists")
    }

    pub fn play(&self) -> Option<PlayRef<'_>> {
        self.position().play()
    }

    pub fn at(&self, id: PositionId) -> Option<CursorRef<'_>> {
        let game = self.game();
        game.tree.contains(id).then(|| Cursor {
            game,
            current: id,
            preserve: self.preserve.clone(),
        })
    }
}

/// Access API.
impl<G: BorrowMut<Game>> Cursor<G> {
    pub fn position_mut(&mut self) -> PositionMut<'_> {
        let id = self.current;
        let protected = self.protected();
        PositionMut { game: self.game.borrow_mut(), id, protected }
    }

    pub fn play_mut(&mut self) -> Option<PlayMut<'_>> {
        self.position_mut().into_play()
    }

    pub fn at_mut(&mut self, id: PositionId) -> Option<CursorMut<'_>> {
        if !self.game.borrow().tree.contains(id) {
            return None;
        }

        let preserve = self.protected().unremovable;
        Some(Cursor { game: self.game.borrow_mut(), current: id, preserve })
    }

    fn protected(&self) -> Protected {
        let mut unremovable = self.preserve.clone();
        unremovable.insert(self.current);
        Protected { unremovable }
    }
}

/// Traversal API.
impl<G: Borrow<Game>> Cursor<G> {
    pub fn set(&mut self, id: PositionId) -> Option<bool> {
        if !self.game.borrow().tree.contains(id) {
            return None;
        }

        let changed = self.current != id;
        self.current = id;
        Some(changed)
    }

    pub fn prev(&mut self) -> Option<PositionId> {
        let current = self.position().play()?.previous().id();
        self.current = current;
        Some(current)
    }

    pub fn main(&mut self) -> Option<PositionId> {
        self.option(0)
    }

    pub fn option(&mut self, option: impl Locate) -> Option<PositionId> {
        let current = self.position().get(option)?.position().id();
        self.current = current;
        Some(current)
    }
}

/// Access API.
impl<'g> CursorRef<'g> {
    pub fn into_game(self) -> &'g Game {
        self.game
    }

    pub fn into_position(self) -> PositionRef<'g> {
        self.game.position(self.current).expect("cursor position exists")
    }

    pub fn into_play(self) -> Option<PlayRef<'g>> {
        self.into_position().play()
    }
}

/// Access API.
impl<'g> CursorMut<'g> {
    pub fn into_position_mut(self) -> PositionMut<'g> {
        let id = self.current;
        let mut unremovable = self.preserve;
        unremovable.insert(id);
        PositionMut { game: self.game, id, protected: Protected { unremovable } }
    }

    pub fn into_play_mut(self) -> Option<PlayMut<'g>> {
        self.into_position_mut().into_play()
    }
}
