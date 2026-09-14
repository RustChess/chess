use core::{cmp::Ordering, iter::FusedIterator, ops::Deref, slice};

use super::*;

/// The partial order of a position within a game tree.
#[derive(Clone, Copy)]
pub struct Order<'g> {
    game: &'g Game,
    pub id: PositionId,
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

    pub fn new_game(self) -> Game {
        Game::unchecked_with_mode(self.position().position(), self.game.mode())
    }

    pub fn index(&self, option: impl Locate) -> Option<usize> {
        let index = option.locate(*self)?;
        (index < self.options().len()).then_some(index)
    }

    pub fn contains(&self, option: impl Locate) -> bool {
        self.index(option).is_some()
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

    pub const fn play(&self) -> Option<PlayRef<'g>> {
        match self.id {
            PositionId::Play(id) => Some(PlayRef { game: self.game, id }),
            PositionId::Start => None,
        }
    }

    pub fn main(&self) -> Option<PlayRef<'g>> {
        self.option(0)
    }

    pub fn option(&self, option: impl Locate) -> Option<PlayRef<'g>> {
        let index = self.index(option)?;
        let id = self.position().options.get(index).copied()?;
        Some(PlayRef { game: self.game, id })
    }

    pub fn has_options(&self) -> bool {
        !self.position().options.is_empty()
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

    pub fn public_mut(&mut self) -> &mut PositionPublic {
        &mut self.position_mut().public
    }

    pub fn comment_mut(&mut self) -> &mut Option<Text> {
        &mut self.position_mut().public.comment
    }

    pub fn evaluation_mut(&mut self) -> &mut Option<Evaluation> {
        &mut self.position_mut().public.evaluation
    }

    pub fn expanded_mut(&mut self) -> &mut bool {
        &mut self.position_mut().public.expanded
    }

    pub fn set_evaluation(&mut self, evaluation: Option<Evaluation>) -> bool {
        if self.public.evaluation != evaluation {
            self.public_mut().evaluation = evaluation;
            true
        } else {
            false
        }
    }

    // Only set if we have at least same depth evaluation.
    // For Multi-PV lines, they are not stable for a given depth
    // until all lines at that depth were determined, hence `<`, not `<=`
    pub fn update_evaluation(&mut self, evaluation: Evaluation) -> bool {
        if self.public.evaluation.is_some_and(|previous| evaluation.depth < previous.depth) {
            false
        } else {
            self.set_evaluation(Some(evaluation))
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

    pub fn main_mut(&mut self) -> Option<PlayMut<'_>> {
        self.option_mut(0)
    }

    pub fn into_main(self) -> Option<PlayMut<'g>> {
        self.into_option(0)
    }

    pub fn option_mut(&mut self, option: impl Locate) -> Option<PlayMut<'_>> {
        let id = self.as_ref().option(option)?.id();
        Some(PlayMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_option(self, option: impl Locate) -> Option<PlayMut<'g>> {
        let id = self.as_ref().option(option)?.id();
        Some(PlayMut { game: self.game, id, protected: self.protected })
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

impl Deref for PositionMut<'_> {
    type Target = Position;

    fn deref(&self) -> &Position {
        self.position()
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

    pub fn comment(&self) -> Option<&'g Text> {
        self.game.tree.position(self.id.into()).comment()
    }

    pub fn evaluation(&self) -> Option<Evaluation> {
        self.game.tree.position(self.id.into()).evaluation()
    }

    pub fn expanded(&self) -> bool {
        self.game.tree.position(self.id.into()).expanded()
    }

    pub(in crate::game) fn play(&self) -> &Play {
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
    const fn new(game: &'g mut Game, id: PlayId) -> Self {
        Self { game, id, protected: Protected::new() }
    }

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

    pub fn public_mut(&mut self) -> &mut PlayPublic {
        &mut self.play_mut().public
    }

    pub fn affixes_mut(&mut self) -> &mut Affixes {
        &mut self.play_mut().public.affixes
    }

    pub fn commands_mut(&mut self) -> &mut Vec<Command> {
        &mut self.play_mut().public.commands
    }

    pub fn nags_mut(&mut self) -> &mut Vec<Nag> {
        &mut self.play_mut().public.nags
    }

    pub fn comment(&self) -> Option<&Text> {
        self.game.tree.position(self.id.into()).comment()
    }

    pub fn evaluation(&self) -> Option<Evaluation> {
        self.as_ref().evaluation()
    }

    pub fn expanded(&self) -> bool {
        self.as_ref().expanded()
    }

    pub fn position_public_mut(&mut self) -> &mut PositionPublic {
        &mut self.game.tree.position_mut(self.id.into()).public
    }

    pub fn comment_mut(&mut self) -> &mut Option<Text> {
        &mut self.position_public_mut().comment
    }

    pub fn evaluation_mut(&mut self) -> &mut Option<Evaluation> {
        &mut self.position_public_mut().evaluation
    }

    pub fn expanded_mut(&mut self) -> &mut bool {
        &mut self.position_public_mut().expanded
    }

    pub(in crate::game) fn play(&self) -> &Play {
        self.game.tree.play(self.id)
    }

    pub(in crate::game) fn play_mut(&mut self) -> &mut Play {
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
    fn locate(self, position: PositionRef<'_>) -> Option<usize> {
        (self < position.options().len()).then_some(self)
    }
}

impl Locate for PlayId {
    fn locate(self, position: PositionRef<'_>) -> Option<usize> {
        position.options().iter().position(|id| *id == self)
    }
}

impl Locate for chess::Move {
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
