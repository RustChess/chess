use core::{
    borrow::{Borrow, BorrowMut},
    ops::Deref,
};

use super::*;

// Note: We have three types of cursor, owning Game, &mut Game, &Game,
// named Cursor, CursorMut, CursorRef.
//
// Implementations on G: Borrow<Game> apply to Cursor and CursorRef
// Implementations on G: BorrowMut<Game> apply to Cursor and CursorMut

/// Access API.
impl Cursor<Game> {
    pub const fn new(game: Game) -> Self {
        Self { game, id: START, preserve: Set::new() }
    }

    pub fn into_game(self) -> Game {
        self.game
    }
}

/// Access API.
impl<G> Cursor<G> {
    pub const fn id(&self) -> PositionId {
        self.id
    }
}

/// Access API.
impl<G: Borrow<Game>> Cursor<G> {
    pub fn game(&self) -> &Game {
        self.game.borrow()
    }

    pub fn position(&self) -> PositionRef<'_> {
        PositionRef { game: self.game(), id: self.id }
    }

    pub fn play(&self) -> Option<PlayRef<'_>> {
        self.position().play()
    }

    pub fn at(&self, id: PositionId) -> Option<CursorRef<'_>> {
        let game = self.game();
        game.tree.contains(id).then(|| Cursor { game, id, preserve: self.preserve.clone() })
    }
}

impl<G: Borrow<Game>> Deref for Cursor<G> {
    type Target = Game;

    fn deref(&self) -> &Game {
        self.game()
    }
}

/// Access API.
impl<G: BorrowMut<Game>> Cursor<G> {
    pub fn orientation_mut(&mut self) -> &mut Player {
        &mut self.game.borrow_mut().orientation
    }

    pub fn outcome_mut(&mut self) -> &mut Outcome {
        &mut self.game.borrow_mut().outcome
    }

    pub fn roster_mut(&mut self) -> &mut Roster {
        &mut self.game.borrow_mut().roster
    }

    pub fn tags_mut(&mut self) -> &mut Vec<Tag> {
        &mut self.game.borrow_mut().tags
    }

    // "reborrowing" clone
    pub fn cursor_mut(&mut self) -> CursorMut<'_> {
        let id = self.id;
        let preserve = self.protected().unremovable;
        Cursor { game: self.game.borrow_mut(), id, preserve }
    }

    pub fn position_mut(&mut self) -> PositionMut<'_> {
        let id = self.id;
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
        Some(Cursor { game: self.game.borrow_mut(), id, preserve })
    }

    fn protected(&self) -> Protected {
        let mut unremovable = self.preserve.clone();
        unremovable.insert(self.id);
        Protected { unremovable }
    }
}

/// Access API.
impl<'g> CursorRef<'g> {
    pub fn into_game(self) -> &'g Game {
        self.game
    }

    pub fn into_position(self) -> PositionRef<'g> {
        PositionRef { game: self.game, id: self.id }
    }

    pub fn into_play(self) -> Option<PlayRef<'g>> {
        self.into_position().play()
    }
}

/// Access API.
impl<'g> CursorMut<'g> {
    pub fn into_position_mut(self) -> PositionMut<'g> {
        let id = self.id;
        let mut unremovable = self.preserve;
        unremovable.insert(id);
        PositionMut { game: self.game, id, protected: Protected { unremovable } }
    }

    pub fn into_play_mut(self) -> Option<PlayMut<'g>> {
        self.into_position_mut().into_play()
    }
}

/// Traversal API.
impl<G: Borrow<Game>> Cursor<G> {
    pub fn set(&mut self, id: PositionId) -> Option<bool> {
        if !self.game.borrow().tree.contains(id) {
            return None;
        }

        let changed = self.id != id;
        self.id = id;
        Some(changed)
    }

    pub fn prev(&mut self) -> Option<PositionId> {
        let id = self.position().play()?.previous().id();
        self.id = id;
        Some(id)
    }

    pub fn mainline_end(&mut self) -> PositionId {
        while self.main().is_some() {}
        self.id
    }

    pub fn main(&mut self) -> Option<PositionId> {
        self.option(0)
    }

    pub fn option(&mut self, option: impl Locate) -> Option<PositionId> {
        let id = self.position().option(option)?.position().id();
        self.id = id;
        Some(id)
    }
}

/// Options Mutation API
impl<G: BorrowMut<Game>> Cursor<G> {
    pub fn option_mut_or_push(&mut self, play: chess::Move) -> Result<PositionId> {
        let id = self.position_mut().option_mut_or_push(play)?.id().into();
        self.id = id;
        Ok(id)
    }

    pub fn option_mut_or_insert_before(
        &mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> Result<PositionId> {
        let id = self.position_mut().option_mut_or_insert_before(option, play)?.id().into();
        self.id = id;
        Ok(id)
    }

    pub fn option_mut_or_insert_after(
        &mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> Result<PositionId> {
        let id = self.position_mut().option_mut_or_insert_after(option, play)?.id().into();
        self.id = id;
        Ok(id)
    }

    /// Unlike `self.position_mut().into_remove_play()`, this first retreats the cursor so its
    /// former position is no longer protected by the cursor itself.
    pub fn remove_play(&mut self) -> Result<()> {
        let play = self.play().ok_or(Error::Missing)?;
        let removed = self.id;
        let id = play.id();
        self.id = play.previous().id();

        if let Err(error) = self.position_mut().remove(id) {
            self.id = removed;
            return Err(error);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{Role::Pawn, Square::*};

    use super::*;

    #[test]
    fn cursor_traversal() {
        let mut cursor = Cursor::new(Game::new(chess::Position::start()));
        let e4 = chess::Move::normal(Pawn, E2, E4);
        let e5 = chess::Move::normal(Pawn, E7, E5);

        assert_eq!(cursor.id(), START);
        assert_eq!(cursor.prev(), None);
        assert_eq!(cursor.main(), None);

        cursor.option_mut_or_push(e4).unwrap();
        let after_e4 = cursor.id();
        cursor.option_mut_or_push(e5).unwrap();
        let after_e5 = cursor.id();

        assert_eq!(cursor.prev(), Some(after_e4));
        assert_eq!(cursor.main(), Some(after_e5));
        assert_eq!(cursor.set(after_e5), Some(false));
        assert_eq!(cursor.play().map(|play| **play), Some(e5));
    }

    #[test]
    fn cursor_removal_protection() {
        let mut game = Game::new(chess::Position::start());
        let e4 = game.start_mut().into_push(chess::Move::normal(Pawn, E2, E4)).unwrap().id();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .into_position()
            .into_push(chess::Move::normal(Pawn, E7, E5))
            .unwrap()
            .id();
        let d4 = game.start_mut().into_push(chess::Move::normal(Pawn, D2, D4)).unwrap().id();

        let mut cursor = Cursor::new(game);
        assert_eq!(cursor.set(e5.into()), Some(true));

        let mut root = cursor.at_mut(START).unwrap();
        let mut start = root.position_mut();
        assert!(matches!(start.remove(e4), Err(Error::Unremovable(id)) if id == e5.into()));
        assert!(start.remove(d4).is_ok());

        assert_eq!(cursor.id(), e5.into());
        assert_eq!(cursor.position().id(), e5.into());
        assert!(cursor.game().play(e4).is_some());
        assert!(cursor.game().play(e5).is_some());
        assert!(cursor.game().play(d4).is_none());
    }

    #[test]
    fn cursor_remove_play() {
        let mut cursor = Cursor::new(Game::new(chess::Position::start()));
        let e4 = cursor.option_mut_or_push(chess::Move::normal(Pawn, E2, E4)).unwrap();
        let e5 = cursor.option_mut_or_push(chess::Move::normal(Pawn, E7, E5)).unwrap();

        assert!(cursor.position_mut().into_remove_play().is_err());
        assert_eq!(cursor.id(), e5);
        assert!(cursor.remove_play().is_ok());
        assert_eq!(cursor.id(), e4);
        assert!(cursor.game().position(e5).is_none());
        assert!(cursor.remove_play().is_ok());
        assert_eq!(cursor.id(), START);
        assert!(matches!(cursor.remove_play(), Err(Error::Missing)));
    }
}
