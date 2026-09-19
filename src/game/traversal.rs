use core::{cmp::Ordering, fmt, iter::FusedIterator, ops::Deref, slice};

use super::*;

/// An error from traversing a game tree.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("position {0:?} not found")]
    Position(PositionId),
    #[error("move {0:?} not found")]
    Move(MoveId),
    #[error(transparent)]
    Option(#[from] MissingOption),
}

/// A result of traversing a game tree.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Locates an option within a game position.
pub trait Locate: Into<Location> {
    fn locate(self, position: PositionRef<'_>) -> Result<usize, MissingOption>;
}

/// A way to locate an option within a position.
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Location {
    Index(usize),
    Id(MoveId),
    Move(chess::Move),
}

/// An option location that does not exist in a position.
#[derive(Debug, thiserror::Error)]
#[error("option {location:?} not found at position {position:?}")]
pub struct MissingOption {
    pub position: PositionId,
    pub location: Location,
}

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Index(index) => write!(f, "{index}"),
            Self::Id(id) => write!(f, "{id:?}"),
            Self::Move(play) => write!(f, "{play}"),
        }
    }
}

impl From<usize> for Location {
    fn from(index: usize) -> Self {
        Self::Index(index)
    }
}

impl From<MoveId> for Location {
    fn from(id: MoveId) -> Self {
        Self::Id(id)
    }
}

impl From<chess::Move> for Location {
    fn from(play: chess::Move) -> Self {
        Self::Move(play)
    }
}

/// The partial order of a position within a game tree.
#[derive(Clone, Copy)]
pub struct Order<'g> {
    game: &'g Game,
    pub id: PositionId,
}

/// Iterator over the moves from a position.
#[derive(Clone)]
pub struct Moves<'g> {
    game: &'g Game,
    ids: slice::Iter<'g, MoveId>,
}

/// Lending iterator over the options from a position in index order.
///
/// Options removed before being reached are skipped.
pub struct MovesMut<'g> {
    game: &'g mut Game,
    position: PositionId,
    index: usize,
    visited: Set<MoveId>,
    protected: Protected,
}

impl Game {
    pub fn play(&self, id: MoveId) -> Result<MoveRef<'_>> {
        self.tree.contains(id.into()).then_some(MoveRef { game: self, id }).ok_or(Error::Move(id))
    }

    pub fn play_mut(&mut self, id: MoveId) -> Result<MoveMut<'_>> {
        if !self.tree.contains(id.into()) {
            return Err(Error::Move(id));
        }
        Ok(MoveMut::new(self, id))
    }

    pub fn position(&self, id: PositionId) -> Result<PositionRef<'_>> {
        self.tree.contains(id).then_some(PositionRef { game: self, id }).ok_or(Error::Position(id))
    }

    pub fn position_mut(&mut self, id: PositionId) -> Result<PositionMut<'_>> {
        if !self.tree.contains(id) {
            return Err(Error::Position(id));
        }
        Ok(PositionMut::new(self, id))
    }

    pub const fn start(&self) -> PositionRef<'_> {
        PositionRef { game: self, id: START }
    }

    pub const fn start_mut(&mut self) -> PositionMut<'_> {
        PositionMut::new(self, START)
    }

    pub const fn into_cursor(self) -> Cursor {
        Cursor::new(self)
    }

    pub const fn cursor(&self) -> CursorRef<'_> {
        Cursor { game: self, id: START, preserve: Set::new() }
    }

    pub const fn cursor_mut(&mut self) -> CursorMut<'_> {
        Cursor { game: self, id: START, preserve: Set::new() }
    }

    pub const fn order(&self, id: PositionId) -> Order<'_> {
        Order { game: self, id }
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

    pub fn into_game(self) -> Game {
        Game::unchecked_with_mode_next(
            self.position().position(),
            self.game.mode(),
            self.game.tree.next(),
        )
    }

    pub fn index(&self, option: impl Locate) -> Result<usize> {
        Ok(option.locate(*self)?)
    }

    pub fn has_option(&self, option: impl Locate) -> bool {
        self.index(option).is_ok()
    }

    pub(in crate::game) fn position(&self) -> &'g Position {
        self.game.tree.position(self.id)
    }
}

/// Traversal API.
impl<'g> PositionRef<'g> {
    pub const fn cursor(self) -> CursorRef<'g> {
        Cursor { game: self.game, id: self.id, preserve: Set::new() }
    }

    pub const fn play(&self) -> start::Result<MoveRef<'g>> {
        match self.id {
            PositionId::Move(id) => Ok(MoveRef { game: self.game, id }),
            PositionId::Start => Err(start::Error),
        }
    }

    pub fn main(&self) -> Result<MoveRef<'g>> {
        self.option(0)
    }

    pub fn option(&self, option: impl Locate) -> Result<MoveRef<'g>> {
        let index = self.index(option)?;
        let id = self.position().options[index];
        Ok(MoveRef { game: self.game, id })
    }

    pub fn has_options(&self) -> bool {
        !self.position().options.is_empty()
    }

    pub fn has_alternatives(&self) -> bool {
        self.position().options.len() >= 2
    }

    pub fn alternatives(&self) -> Moves<'g> {
        let ids = self.position().options.get(1..).unwrap_or_default().iter();
        Moves { game: self.game, ids }
    }

    pub fn iter(&self) -> Moves<'g> {
        Moves { game: self.game, ids: self.position().options.iter() }
    }
}

impl Deref for PositionRef<'_> {
    type Target = Position;

    fn deref(&self) -> &Position {
        self.position()
    }
}

/// Access API.
impl<'g> PositionMut<'g> {
    const fn new(game: &'g mut Game, id: PositionId) -> Self {
        Self { game, id, protected: Protected::new() }
    }

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

    pub fn set_comment(&mut self, comment: Option<Text>) {
        self.update_public(|public| public.comment = comment);
    }

    pub fn set_valuation(&mut self, valuation: Option<Valuation>) {
        self.update_public(|public| public.valuation = valuation);
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.update_public(|public| public.expanded = expanded);
    }

    // Only set if we have at least the same valuation depth.
    // For Multi-PV lines, they are not stable for a given depth
    // until all lines at that depth were determined, hence `<`, not `<=`
    pub fn update_valuation(&mut self, valuation: Valuation) -> bool {
        if self.public.valuation.is_some_and(|previous| valuation.depth < previous.depth) {
            false
        } else {
            let changed = self.public.valuation != Some(valuation);
            self.set_valuation(Some(valuation));
            changed
        }
    }

    fn update_public(&mut self, update: impl FnOnce(&mut PositionPublic)) {
        let before = self.game.undo.on.then(|| self.public.clone());
        update(&mut self.position_mut().public);
        if let Some(before) = before
            && before != self.public
        {
            self.game.undo.stack.push(patch::Patch::Position { id: self.id, public: before });
        }
    }

    pub(in crate::game) fn position(&self) -> &Position {
        self.game.tree.position(self.id)
    }

    pub(in crate::game) fn position_mut(&mut self) -> &mut Position {
        self.game.tree.position_mut(self.id)
    }
}

/// Traversal API.
impl<'g> PositionMut<'g> {
    pub fn cursor_mut(&mut self) -> CursorMut<'_> {
        let mut preserve = self.protected.unremovable.clone();
        preserve.insert(self.id);
        Cursor { game: self.game, id: self.id, preserve }
    }

    pub fn into_cursor_mut(self) -> CursorMut<'g> {
        Cursor { game: self.game, id: self.id, preserve: self.protected.unremovable }
    }

    pub const fn play(&self) -> start::Result<MoveRef<'_>> {
        match self.id {
            PositionId::Move(id) => Ok(MoveRef { game: self.game, id }),
            PositionId::Start => Err(start::Error),
        }
    }

    pub fn play_mut(&mut self) -> start::Result<MoveMut<'_>> {
        match self.id {
            PositionId::Move(id) => {
                Ok(MoveMut { game: self.game, id, protected: self.protected.clone() })
            }
            PositionId::Start => Err(start::Error),
        }
    }

    pub fn into_play(self) -> start::Result<MoveMut<'g>> {
        match self.id {
            PositionId::Move(id) => Ok(MoveMut { game: self.game, id, protected: self.protected }),
            PositionId::Start => Err(start::Error),
        }
    }

    pub fn main_mut(&mut self) -> Result<MoveMut<'_>> {
        self.option_mut(0)
    }

    pub fn into_main(self) -> Result<MoveMut<'g>> {
        self.into_option(0)
    }

    pub fn option_mut(&mut self, option: impl Locate) -> Result<MoveMut<'_>> {
        let id = self.as_ref().option(option)?.id();
        Ok(MoveMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_option(self, option: impl Locate) -> Result<MoveMut<'g>> {
        let id = self.as_ref().option(option)?.id();
        Ok(MoveMut { game: self.game, id, protected: self.protected })
    }

    pub fn alternatives_mut(&mut self) -> MovesMut<'_> {
        MovesMut {
            game: self.game,
            position: self.id,
            index: 1,
            visited: Set::new(),
            protected: self.protected.clone(),
        }
    }

    pub fn into_alternatives(self) -> MovesMut<'g> {
        MovesMut {
            game: self.game,
            position: self.id,
            index: 1,
            visited: Set::new(),
            protected: self.protected,
        }
    }

    pub fn iter_mut(&mut self) -> MovesMut<'_> {
        MovesMut {
            game: self.game,
            position: self.id,
            index: 0,
            visited: Set::new(),
            protected: self.protected.clone(),
        }
    }

    pub fn into_iter_mut(self) -> MovesMut<'g> {
        MovesMut {
            game: self.game,
            position: self.id,
            index: 0,
            visited: Set::new(),
            protected: self.protected,
        }
    }
}

impl Deref for PositionMut<'_> {
    type Target = Position;

    fn deref(&self) -> &Position {
        self.position()
    }
}

/// Access API.
impl<'g> MoveRef<'g> {
    pub const fn id(&self) -> MoveId {
        self.id
    }

    pub const fn game(&self) -> &'g Game {
        self.game
    }

    pub fn into_game(self) -> Game {
        let mut game = self.previous().into_game();
        game.start_mut()
            .insert_at_untracked(Some(self.id), 0, self.play().play)
            .expect("move is legal in its previous position");
        game
    }

    pub fn comment(&self) -> Option<&'g Text> {
        self.game.tree.position(self.id.into()).comment()
    }

    pub fn valuation(&self) -> Option<Valuation> {
        self.game.tree.position(self.id.into()).valuation()
    }

    pub fn expanded(&self) -> bool {
        self.game.tree.position(self.id.into()).expanded()
    }

    pub(in crate::game) fn play(&self) -> &Move {
        self.game.tree.play(self.id)
    }
}

/// Traversal API.
impl<'g> MoveRef<'g> {
    pub fn previous(&self) -> PositionRef<'g> {
        PositionRef { game: self.game, id: self.play().previous }
    }

    pub const fn position(&self) -> PositionRef<'g> {
        PositionRef { game: self.game, id: PositionId::Move(self.id) }
    }
}

impl Deref for MoveRef<'_> {
    type Target = Move;

    fn deref(&self) -> &Move {
        self.play()
    }
}

/// Access API.
impl<'g> MoveMut<'g> {
    const fn new(game: &'g mut Game, id: MoveId) -> Self {
        Self { game, id, protected: Protected::new() }
    }

    pub const fn id(&self) -> MoveId {
        self.id
    }

    pub const fn game(&self) -> &Game {
        self.game
    }

    pub const fn as_ref(&self) -> MoveRef<'_> {
        MoveRef { game: self.game, id: self.id }
    }

    pub fn into_ref(self) -> MoveRef<'g> {
        MoveRef { game: self.game, id: self.id }
    }

    pub fn set_intro(&mut self, intro: Option<Text>) {
        self.update_public(|public| public.affixes.intro = intro);
    }

    pub fn set_outro(&mut self, outro: Option<Text>) {
        self.update_public(|public| public.affixes.outro = outro);
    }

    pub fn update_commands(&mut self, update: impl FnOnce(&mut Vec<Command>)) {
        self.update_public(|public| update(&mut public.commands));
    }

    pub fn update_nags(&mut self, update: impl FnOnce(&mut Vec<Nag>)) {
        self.update_public(|public| update(&mut public.nags));
    }

    fn update_public(&mut self, update: impl FnOnce(&mut MovePublic)) {
        let before = self.game.undo.on.then(|| self.public.clone());
        update(&mut self.play_mut().public);
        if let Some(before) = before
            && before != self.public
        {
            self.game.undo.stack.push(patch::Patch::Move { id: self.id, public: before });
        }
    }

    pub fn comment(&self) -> Option<&Text> {
        self.game.tree.position(self.id.into()).comment()
    }

    pub fn valuation(&self) -> Option<Valuation> {
        self.as_ref().valuation()
    }

    pub fn expanded(&self) -> bool {
        self.as_ref().expanded()
    }

    pub fn set_comment(&mut self, comment: Option<Text>) {
        self.position_mut().set_comment(comment);
    }

    pub fn set_valuation(&mut self, valuation: Option<Valuation>) {
        self.position_mut().set_valuation(valuation);
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.position_mut().set_expanded(expanded);
    }

    pub(in crate::game) fn play(&self) -> &Move {
        self.game.tree.play(self.id)
    }

    pub(in crate::game) fn play_mut(&mut self) -> &mut Move {
        self.game.tree.play_mut(self.id)
    }
}

/// Traversal API.
impl<'g> MoveMut<'g> {
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
        PositionRef { game: self.game, id: PositionId::Move(self.id) }
    }

    pub fn position_mut(&mut self) -> PositionMut<'_> {
        PositionMut {
            game: self.game,
            id: PositionId::Move(self.id),
            protected: self.protected.clone(),
        }
    }

    pub fn into_position(self) -> PositionMut<'g> {
        PositionMut { game: self.game, id: PositionId::Move(self.id), protected: self.protected }
    }
}

impl Deref for MoveMut<'_> {
    type Target = Move;

    fn deref(&self) -> &Move {
        self.play()
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

impl Locate for usize {
    fn locate(self, position: PositionRef<'_>) -> Result<usize, MissingOption> {
        (self < position.options().len())
            .then_some(self)
            .ok_or(MissingOption { position: position.id(), location: self.into() })
    }
}

impl Locate for MoveId {
    fn locate(self, position: PositionRef<'_>) -> Result<usize, MissingOption> {
        position
            .options()
            .iter()
            .position(|option| *option == self)
            .ok_or(MissingOption { position: position.id(), location: self.into() })
    }
}

impl Locate for chess::Move {
    fn locate(self, position: PositionRef<'_>) -> Result<usize, MissingOption> {
        position
            .iter()
            .position(|option| **option == self)
            .ok_or(MissingOption { position: position.id(), location: self.into() })
    }
}

impl<'g> IntoIterator for PositionRef<'g> {
    type Item = MoveRef<'g>;
    type IntoIter = Moves<'g>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'g> Iterator for Moves<'g> {
    type Item = MoveRef<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.ids.next().copied()?;
        Some(MoveRef { game: self.game, id })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.ids.size_hint()
    }
}

impl DoubleEndedIterator for Moves<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let id = self.ids.next_back().copied()?;
        Some(MoveRef { game: self.game, id })
    }
}

impl ExactSizeIterator for Moves<'_> {}
impl FusedIterator for Moves<'_> {}

impl MovesMut<'_> {
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<MoveMut<'_>> {
        loop {
            let id = self.game.tree.position(self.position).options.get(self.index).copied()?;
            self.index += 1;

            if self.visited.insert(id) && self.game.tree.contains(id.into()) {
                return Some(MoveMut { game: self.game, id, protected: self.protected.clone() });
            }
        }
    }
}
