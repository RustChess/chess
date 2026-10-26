use core::{cmp::Ordering, fmt, iter::FusedIterator, ops::Deref, slice};

use std::collections::VecDeque;

use crate::position::Analysis;

use super::*;

/// An error from traversing a game tree.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("position slot {0} not found")]
    Position(PositionSlot),
    #[error("move slot {0} not found")]
    Move(MoveSlot),
    #[error(transparent)]
    Option(#[from] Missing),
}

/// A result of traversing a game tree.
pub type Result<T, E = Error> = core::result::Result<T, E>;

/// Chooses an option within a game position.
pub trait Choose: Into<Choice> {
    fn choose(self, position: PositionRef<'_>) -> Result<usize, Missing>;
}

/// Concrete instantiation of any Choose type.
// This is a "mini-version" of the enum/trait bridge,
// as exemplified in the `enum-trait` crate.
#[derive(Clone, Copy, Eq, PartialEq)]
#[non_exhaustive]
pub enum Choice {
    Index(usize),
    Move(chess::Move),
    Slot(MoveSlot),
}

/// A choice that does not identify an existing option in a position.
#[derive(Debug, thiserror::Error)]
#[error("option {choice:?} not found in position slot {position}")]
pub struct Missing {
    pub position: PositionSlot,
    pub choice: Choice,
}

/// A position at a tree distance from an origin.
#[derive(Clone, Copy)]
pub struct Near<'g> {
    /// Number of tree edges from the origin.
    pub distance: usize,
    /// Position at this distance.
    pub position: PositionRef<'g>,
}

/// Positions in ascending tree distance from an origin.
pub struct Nearby<'g> {
    game: &'g Game,
    queue: VecDeque<(PositionSlot, usize)>,
    visited: Set<PositionSlot>,
}

impl fmt::Debug for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Index(index) => write!(f, "Index({index})"),
            Self::Move(play) => write!(f, "Move({play})"),
            Self::Slot(slot) => write!(f, "Slot({slot})"),
        }
    }
}

impl From<usize> for Choice {
    fn from(index: usize) -> Self {
        Self::Index(index)
    }
}

impl From<MoveSlot> for Choice {
    fn from(slot: MoveSlot) -> Self {
        Self::Slot(slot)
    }
}

impl From<chess::Move> for Choice {
    fn from(play: chess::Move) -> Self {
        Self::Move(play)
    }
}

/// The partial order of a position within a game tree.
#[derive(Clone, Copy)]
pub struct Order<'g> {
    game: &'g Game,
    pub slot: PositionSlot,
}

/// Iterator over the moves from a position.
#[derive(Clone)]
pub struct Moves<'g> {
    game: &'g Game,
    slots: slice::Iter<'g, MoveSlot>,
}

/// Lending iterator over the options from a position in index order.
///
/// Options removed before being reached are skipped.
pub struct MovesMut<'g> {
    game: &'g mut Game,
    position: PositionSlot,
    index: usize,
    visited: Set<MoveSlot>,
    protected: Protected,
}

impl Game {
    pub fn mainline(&self) -> Line<chess::Position, chess::Move> {
        let mut cursor = self.cursor();
        cursor.mainline_end();
        cursor.position().line()
    }

    pub fn distance(&self, a: PositionSlot, b: PositionSlot) -> Result<usize> {
        self.position(a)?;
        self.position(b)?;

        let mut ancestors = Map::new();
        let mut distance = 0;
        let mut slot = Some(a);
        while let Some(ancestor) = slot {
            ancestors.insert(ancestor, distance);
            distance += 1;
            slot = self.previous(ancestor);
        }

        distance = 0;
        slot = Some(b);
        while let Some(ancestor) = slot {
            if let Some(a_distance) = ancestors.get(&ancestor) {
                return Ok(a_distance + distance);
            }
            distance += 1;
            slot = self.previous(ancestor);
        }

        unreachable!("positions in a game share the start position")
    }

    pub fn nearby(&self, origin: PositionSlot) -> Result<Nearby<'_>> {
        self.position(origin)?;
        Ok(Nearby {
            game: self,
            queue: VecDeque::from([(origin, 0)]),
            visited: Set::from([origin]),
        })
    }

    pub fn play(&self, slot: MoveSlot) -> Result<MoveRef<'_>> {
        self.tree
            .contains(slot.into())
            .then_some(MoveRef { game: self, slot })
            .ok_or(Error::Move(slot))
    }

    pub fn play_mut(&mut self, slot: MoveSlot) -> Result<MoveMut<'_>> {
        if !self.tree.contains(slot.into()) {
            return Err(Error::Move(slot));
        }
        Ok(MoveMut::new(self, slot))
    }

    pub fn position(&self, slot: PositionSlot) -> Result<PositionRef<'_>> {
        self.tree
            .contains(slot)
            .then_some(PositionRef { game: self, slot })
            .ok_or(Error::Position(slot))
    }

    pub fn position_mut(&mut self, slot: PositionSlot) -> Result<PositionMut<'_>> {
        if !self.tree.contains(slot) {
            return Err(Error::Position(slot));
        }
        Ok(PositionMut::new(self, slot))
    }

    pub const fn start(&self) -> PositionRef<'_> {
        PositionRef { game: self, slot: START }
    }

    pub const fn start_mut(&mut self) -> PositionMut<'_> {
        PositionMut::new(self, START)
    }

    pub const fn into_cursor(self) -> Cursor {
        Cursor::new(self)
    }

    pub const fn cursor(&self) -> CursorRef<'_> {
        Cursor { game: self, slot: START, preserve: Set::new() }
    }

    pub const fn cursor_mut(&mut self) -> CursorMut<'_> {
        Cursor { game: self, slot: START, preserve: Set::new() }
    }

    pub const fn order(&self, slot: PositionSlot) -> Order<'_> {
        Order { game: self, slot }
    }

    pub fn previous(&self, slot: PositionSlot) -> Option<PositionSlot> {
        match slot {
            PositionSlot::Start => None,
            PositionSlot::Move(slot) => Some(self.tree.play(slot).previous),
        }
    }
}

/// Access API.
impl<'g> PositionRef<'g> {
    pub const fn slot(&self) -> PositionSlot {
        self.slot
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

    pub fn index(&self, option: impl Choose) -> Result<usize> {
        Ok(option.choose(*self)?)
    }

    pub fn has_option(&self, option: impl Choose) -> bool {
        self.index(option).is_ok()
    }

    pub(in crate::game) fn position(&self) -> &'g Position {
        self.game.tree.position(self.slot)
    }
}

/// Traversal API.
impl<'g> PositionRef<'g> {
    pub fn line(self) -> Line<chess::Position, chess::Move> {
        let position = **self.game.start();
        let mut current = self;
        let mut moves = Vec::new();

        while let Ok(play) = current.play() {
            moves.push(**play);
            current = play.previous();
        }
        moves.reverse();

        Line { position, moves }
    }

    pub const fn cursor(self) -> CursorRef<'g> {
        Cursor { game: self.game, slot: self.slot, preserve: Set::new() }
    }

    pub const fn play(&self) -> start::Result<MoveRef<'g>> {
        match self.slot {
            PositionSlot::Move(slot) => Ok(MoveRef { game: self.game, slot }),
            PositionSlot::Start => Err(start::Error),
        }
    }

    pub fn main(&self) -> Result<MoveRef<'g>> {
        self.option(0)
    }

    pub fn option(&self, option: impl Choose) -> Result<MoveRef<'g>> {
        let index = self.index(option)?;
        let slot = self.position().options[index];
        Ok(MoveRef { game: self.game, slot })
    }

    pub fn has_options(&self) -> bool {
        !self.position().options.is_empty()
    }

    pub fn has_alternatives(&self) -> bool {
        self.position().options.len() >= 2
    }

    pub fn alternatives(&self) -> Moves<'g> {
        let slots = self.position().options.get(1..).unwrap_or_default().iter();
        Moves { game: self.game, slots }
    }

    pub fn iter(&self) -> Moves<'g> {
        Moves { game: self.game, slots: self.position().options.iter() }
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
    const fn new(game: &'g mut Game, slot: PositionSlot) -> Self {
        Self { game, slot, protected: Protected::new() }
    }

    pub const fn slot(&self) -> PositionSlot {
        self.slot
    }

    pub const fn game(&self) -> &Game {
        self.game
    }

    pub const fn as_ref(&self) -> PositionRef<'_> {
        PositionRef { game: self.game, slot: self.slot }
    }

    pub fn into_ref(self) -> PositionRef<'g> {
        PositionRef { game: self.game, slot: self.slot }
    }

    pub fn update_arrows(&mut self, update: impl FnOnce(&mut Vec<Arrow>)) {
        self.update_public(|public| update(&mut public.arrows));
    }

    pub fn set_comment(&mut self, comment: Option<Text>) {
        self.update_public(|public| public.comment = comment);
    }

    pub fn update_commands(&mut self, update: impl FnOnce(&mut Vec<command::Other>)) {
        self.update_public(|public| update(&mut public.commands));
    }

    pub fn set_analysis(&mut self, analysis: Option<Analysis>) {
        self.position_mut().analysis = analysis;
    }

    /// Replaces the analysis if the new evaluation improves the current one.
    pub fn update_analysis(&mut self, analysis: Analysis) -> bool {
        if self.analysis().is_some_and(|current| {
            !matches!(
                (&analysis, current),
                (Analysis::Evaluation(new), Analysis::Evaluation(current))
                    if new.improves(current)
            )
        }) {
            return false;
        }
        self.set_analysis(Some(analysis));
        true
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.update_public(|public| public.expanded = expanded);
    }

    pub fn update_markers(&mut self, update: impl FnOnce(&mut Vec<Marker>)) {
        self.update_public(|public| update(&mut public.markers));
    }

    pub fn update_public(&mut self, update: impl FnOnce(&mut PositionPublic)) {
        let before = self.game.tracks_changes().then(|| self.public.clone());
        update(&mut self.position_mut().public);
        if let Some(before) = before
            && before != self.public
        {
            self.game.track(change::Patch::Position { slot: self.slot, public: Box::new(before) });
        }
    }

    pub(in crate::game) fn position(&self) -> &Position {
        self.game.tree.position(self.slot)
    }

    pub(in crate::game) fn position_mut(&mut self) -> &mut Position {
        self.game.tree.position_mut(self.slot)
    }
}

/// Traversal API.
impl<'g> PositionMut<'g> {
    pub fn cursor_mut(&mut self) -> CursorMut<'_> {
        let mut preserve = self.protected.unremovable.clone();
        preserve.insert(self.slot);
        Cursor { game: self.game, slot: self.slot, preserve }
    }

    pub fn into_cursor_mut(self) -> CursorMut<'g> {
        Cursor { game: self.game, slot: self.slot, preserve: self.protected.unremovable }
    }

    pub const fn play(&self) -> start::Result<MoveRef<'_>> {
        match self.slot {
            PositionSlot::Move(slot) => Ok(MoveRef { game: self.game, slot }),
            PositionSlot::Start => Err(start::Error),
        }
    }

    pub fn play_mut(&mut self) -> start::Result<MoveMut<'_>> {
        match self.slot {
            PositionSlot::Move(slot) => {
                Ok(MoveMut { game: self.game, slot, protected: self.protected.clone() })
            }
            PositionSlot::Start => Err(start::Error),
        }
    }

    pub fn into_play(self) -> start::Result<MoveMut<'g>> {
        match self.slot {
            PositionSlot::Move(slot) => {
                Ok(MoveMut { game: self.game, slot, protected: self.protected })
            }
            PositionSlot::Start => Err(start::Error),
        }
    }

    pub fn main_mut(&mut self) -> Result<MoveMut<'_>> {
        self.option_mut(0)
    }

    pub fn into_main(self) -> Result<MoveMut<'g>> {
        self.into_option(0)
    }

    pub fn option_mut(&mut self, option: impl Choose) -> Result<MoveMut<'_>> {
        let slot = self.as_ref().option(option)?.slot();
        Ok(MoveMut { game: self.game, slot, protected: self.protected.clone() })
    }

    pub fn into_option(self, option: impl Choose) -> Result<MoveMut<'g>> {
        let slot = self.as_ref().option(option)?.slot();
        Ok(MoveMut { game: self.game, slot, protected: self.protected })
    }

    pub fn alternatives_mut(&mut self) -> MovesMut<'_> {
        MovesMut {
            game: self.game,
            position: self.slot,
            index: 1,
            visited: Set::new(),
            protected: self.protected.clone(),
        }
    }

    pub fn into_alternatives(self) -> MovesMut<'g> {
        MovesMut {
            game: self.game,
            position: self.slot,
            index: 1,
            visited: Set::new(),
            protected: self.protected,
        }
    }

    pub fn iter_mut(&mut self) -> MovesMut<'_> {
        MovesMut {
            game: self.game,
            position: self.slot,
            index: 0,
            visited: Set::new(),
            protected: self.protected.clone(),
        }
    }

    pub fn into_iter_mut(self) -> MovesMut<'g> {
        MovesMut {
            game: self.game,
            position: self.slot,
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
    pub const fn slot(&self) -> MoveSlot {
        self.slot
    }

    pub const fn game(&self) -> &'g Game {
        self.game
    }

    pub fn into_game(self) -> Game {
        let mut game = self.previous().into_game();
        game.start_mut()
            .insert_at_untracked(Some(self.slot), 0, self.play().play)
            .expect("move is legal in its previous position");
        game
    }

    pub fn arrows(&self) -> &'g [Arrow] {
        self.game.tree.position(self.slot.into()).arrows()
    }

    pub fn comment(&self) -> Option<&'g Text> {
        self.game.tree.position(self.slot.into()).comment()
    }

    pub fn analysis(&self) -> Option<&'g Analysis> {
        self.game.tree.position(self.slot.into()).analysis()
    }

    pub fn expanded(&self) -> bool {
        self.game.tree.position(self.slot.into()).expanded()
    }

    pub fn markers(&self) -> &'g [Marker] {
        self.game.tree.position(self.slot.into()).markers()
    }

    pub(in crate::game) fn play(&self) -> &Move {
        self.game.tree.play(self.slot)
    }
}

/// Traversal API.
impl<'g> MoveRef<'g> {
    pub fn previous(&self) -> PositionRef<'g> {
        PositionRef { game: self.game, slot: self.play().previous }
    }

    pub const fn position(&self) -> PositionRef<'g> {
        PositionRef { game: self.game, slot: PositionSlot::Move(self.slot) }
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
    const fn new(game: &'g mut Game, slot: MoveSlot) -> Self {
        Self { game, slot, protected: Protected::new() }
    }

    pub const fn slot(&self) -> MoveSlot {
        self.slot
    }

    pub const fn game(&self) -> &Game {
        self.game
    }

    pub const fn as_ref(&self) -> MoveRef<'_> {
        MoveRef { game: self.game, slot: self.slot }
    }

    pub fn into_ref(self) -> MoveRef<'g> {
        MoveRef { game: self.game, slot: self.slot }
    }

    pub fn set_intro(&mut self, intro: Option<Text>) {
        self.update_public(|public| public.affixes.intro = intro);
    }

    pub fn set_outro(&mut self, outro: Option<Text>) {
        self.update_public(|public| public.affixes.outro = outro);
    }

    pub fn update_arrows(&mut self, update: impl FnOnce(&mut Vec<Arrow>)) {
        self.position_mut().update_arrows(update);
    }

    pub fn update_nags(&mut self, update: impl FnOnce(&mut Vec<Nag>)) {
        self.update_public(|public| update(&mut public.nags));
    }

    pub fn update_public(&mut self, update: impl FnOnce(&mut MovePublic)) {
        let before = self.game.tracks_changes().then(|| self.public.clone());
        update(&mut self.play_mut().public);
        if let Some(before) = before
            && before != self.public
        {
            self.game.track(change::Patch::Move { slot: self.slot, public: Box::new(before) });
        }
    }

    pub fn arrows(&self) -> &[Arrow] {
        self.as_ref().arrows()
    }

    pub fn comment(&self) -> Option<&Text> {
        self.game.tree.position(self.slot.into()).comment()
    }

    pub fn analysis(&self) -> Option<&Analysis> {
        self.as_ref().analysis()
    }

    pub fn expanded(&self) -> bool {
        self.as_ref().expanded()
    }

    pub fn markers(&self) -> &[Marker] {
        self.as_ref().markers()
    }

    pub fn set_comment(&mut self, comment: Option<Text>) {
        self.position_mut().set_comment(comment);
    }

    pub fn set_analysis(&mut self, analysis: Option<Analysis>) {
        self.position_mut().set_analysis(analysis);
    }

    /// Replaces the position analysis if the new evaluation improves it.
    pub fn update_analysis(&mut self, analysis: Analysis) -> bool {
        self.position_mut().update_analysis(analysis)
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.position_mut().set_expanded(expanded);
    }

    pub fn update_markers(&mut self, update: impl FnOnce(&mut Vec<Marker>)) {
        self.position_mut().update_markers(update);
    }

    pub(in crate::game) fn play(&self) -> &Move {
        self.game.tree.play(self.slot)
    }

    pub(in crate::game) fn play_mut(&mut self) -> &mut Move {
        self.game.tree.play_mut(self.slot)
    }
}

/// Traversal API.
impl<'g> MoveMut<'g> {
    pub fn previous(&self) -> PositionRef<'_> {
        self.as_ref().previous()
    }

    pub fn previous_mut(&mut self) -> PositionMut<'_> {
        let slot = self.play().previous;
        let protected = self.protected.with_unremovable(self.slot.into());
        PositionMut { game: self.game, slot, protected }
    }

    pub fn into_previous(self) -> PositionMut<'g> {
        let slot = self.play().previous;
        PositionMut { game: self.game, slot, protected: self.protected }
    }

    pub const fn position(&self) -> PositionRef<'_> {
        PositionRef { game: self.game, slot: PositionSlot::Move(self.slot) }
    }

    pub fn position_mut(&mut self) -> PositionMut<'_> {
        PositionMut {
            game: self.game,
            slot: PositionSlot::Move(self.slot),
            protected: self.protected.clone(),
        }
    }

    pub fn into_position(self) -> PositionMut<'g> {
        PositionMut {
            game: self.game,
            slot: PositionSlot::Move(self.slot),
            protected: self.protected,
        }
    }
}

impl Deref for MoveMut<'_> {
    type Target = Move;

    fn deref(&self) -> &Move {
        self.play()
    }
}

impl PartialEq<PositionSlot> for Order<'_> {
    fn eq(&self, other: &PositionSlot) -> bool {
        self.slot == *other
    }
}

impl PartialOrd<PositionSlot> for Order<'_> {
    fn partial_cmp(&self, other: &PositionSlot) -> Option<Ordering> {
        if self.slot == *other {
            return Some(Ordering::Equal);
        }
        if !self.game.tree.contains(self.slot) || !self.game.tree.contains(*other) {
            return None;
        }
        if self.game.tree.less_equal(self.slot, *other) {
            Some(Ordering::Less)
        } else if self.game.tree.less_equal(*other, self.slot) {
            Some(Ordering::Greater)
        } else {
            None
        }
    }
}

impl Choose for usize {
    fn choose(self, position: PositionRef<'_>) -> Result<usize, Missing> {
        (self < position.options().len())
            .then_some(self)
            .ok_or(Missing { position: position.slot(), choice: self.into() })
    }
}

impl Choose for MoveSlot {
    fn choose(self, position: PositionRef<'_>) -> Result<usize, Missing> {
        position
            .options()
            .iter()
            .position(|option| *option == self)
            .ok_or(Missing { position: position.slot(), choice: self.into() })
    }
}

impl Choose for chess::Move {
    fn choose(self, position: PositionRef<'_>) -> Result<usize, Missing> {
        position
            .iter()
            .position(|option| **option == self)
            .ok_or(Missing { position: position.slot(), choice: self.into() })
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
        let slot = self.slots.next().copied()?;
        Some(MoveRef { game: self.game, slot })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.slots.size_hint()
    }
}

impl DoubleEndedIterator for Moves<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        let slot = self.slots.next_back().copied()?;
        Some(MoveRef { game: self.game, slot })
    }
}

impl ExactSizeIterator for Moves<'_> {}
impl FusedIterator for Moves<'_> {}

impl<'g> Iterator for Nearby<'g> {
    type Item = Near<'g>;

    fn next(&mut self) -> Option<Self::Item> {
        let (slot, distance) = self.queue.pop_front()?;

        if let Some(previous) = self.game.previous(slot)
            && self.visited.insert(previous)
        {
            self.queue.push_back((previous, distance + 1));
        }
        for option in self.game.tree.position(slot).options.iter().copied() {
            let position = PositionSlot::Move(option);
            if self.visited.insert(position) {
                self.queue.push_back((position, distance + 1));
            }
        }

        Some(Near { distance, position: PositionRef { game: self.game, slot } })
    }
}

impl FusedIterator for Nearby<'_> {}

impl MovesMut<'_> {
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<MoveMut<'_>> {
        loop {
            let slot = self.game.tree.position(self.position).options.get(self.index).copied()?;
            self.index += 1;

            if self.visited.insert(slot) && self.game.tree.contains(slot.into()) {
                return Some(MoveMut { game: self.game, slot, protected: self.protected.clone() });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Role::{Knight, Pawn},
        Square::*,
    };

    use super::*;

    #[test]
    fn choice_debug() {
        let slot = MoveSlot::from_index(21).unwrap();
        let play = chess::Move::normal(Pawn, E2, E4);

        assert_eq!(format!("{:?}", Choice::Index(37)), "Index(37)");
        assert_eq!(format!("{:?}", Choice::Move(play)), "Move(e2-e4)");
        assert_eq!(format!("{:?}", Choice::Slot(slot)), "Slot(22)");
    }

    #[test]
    fn missing_display() {
        let position = PositionSlot::from(12);
        let slot = MoveSlot::from_index(21).unwrap();
        let play = chess::Move::normal(Pawn, E2, E4);

        assert_eq!(Error::Position(position).to_string(), "position slot 12 not found");
        assert_eq!(Error::Move(slot).to_string(), "move slot 22 not found");
        assert_eq!(
            Missing { position, choice: Choice::Index(37) }.to_string(),
            "option Index(37) not found in position slot 12"
        );
        assert_eq!(
            Missing { position, choice: Choice::Slot(slot) }.to_string(),
            "option Slot(22) not found in position slot 12"
        );
        assert_eq!(
            Missing { position, choice: Choice::Move(play) }.to_string(),
            "option Move(e2-e4) not found in position slot 12"
        );
    }

    #[test]
    fn nearby_positions() {
        let mut game = Game::new(chess::Position::start());
        let e4 = game.start_mut().push(chess::Move::normal(Pawn, E2, E4)).unwrap().slot();
        let d4 = game.start_mut().push(chess::Move::normal(Pawn, D2, D4)).unwrap().slot();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .into_position()
            .push(chess::Move::normal(Pawn, E7, E5))
            .unwrap()
            .slot();
        let c5 = game
            .play_mut(e4)
            .unwrap()
            .into_position()
            .push(chess::Move::normal(Pawn, C7, C5))
            .unwrap()
            .slot();
        let nf3 = game
            .play_mut(e5)
            .unwrap()
            .into_position()
            .push(chess::Move::normal(Knight, G1, F3))
            .unwrap()
            .slot();
        let d5 = game
            .play_mut(d4)
            .unwrap()
            .into_position()
            .push(chess::Move::normal(Pawn, D7, D5))
            .unwrap()
            .slot();

        let nearby = game
            .nearby(e4.into())
            .unwrap()
            .map(|near| (near.position.slot(), near.distance))
            .collect::<Vec<_>>();

        assert_eq!(
            nearby,
            [
                (e4.into(), 0),
                (PositionSlot::Start, 1),
                (e5.into(), 1),
                (c5.into(), 1),
                (d4.into(), 2),
                (nf3.into(), 2),
                (d5.into(), 3),
            ]
        );
        assert_eq!(game.distance(c5.into(), d5.into()).unwrap(), 4);
        assert_eq!(game.distance(nf3.into(), nf3.into()).unwrap(), 0);
    }

    #[test]
    fn nearby_rejects_missing_origin() {
        let game = Game::new(chess::Position::start());
        let missing = PositionSlot::from(1);

        assert!(matches!(game.nearby(missing), Err(Error::Position(slot)) if slot == missing));
        assert!(matches!(
            game.distance(PositionSlot::Start, missing),
            Err(Error::Position(slot)) if slot == missing
        ));
    }
}
