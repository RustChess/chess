use core::{
    borrow::{Borrow, BorrowMut},
    ops::Deref,
};

use super::{options, traversal::Choose, *};

// Note: We have three types of cursor, owning Game, &mut Game, &Game,
// named Cursor, CursorMut, CursorRef.
//
// Implementations on G: Borrow<Game> apply to Cursor and CursorRef
// Implementations on G: BorrowMut<Game> apply to Cursor and CursorMut

/// Access API.
impl Cursor<Game> {
    pub const fn new(game: Game) -> Self {
        Self { game, slot: START, preserve: Set::new() }
    }

    pub fn into_game(self) -> Game {
        self.game
    }
}

/// Access API.
impl<G> Cursor<G> {
    pub const fn slot(&self) -> PositionSlot {
        self.slot
    }
}

/// Access API.
impl<G: Borrow<Game>> Cursor<G> {
    pub fn game(&self) -> &Game {
        self.game.borrow()
    }

    pub fn position(&self) -> PositionRef<'_> {
        PositionRef { game: self.game(), slot: self.slot }
    }

    pub fn play(&self) -> start::Result<MoveRef<'_>> {
        self.position().play()
    }

    pub fn at(&self, slot: PositionSlot) -> traversal::Result<CursorRef<'_>> {
        let game = self.game();
        game.position(slot)?;
        Ok(Cursor { game, slot, preserve: self.preserve.clone() })
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
    pub fn game_mut(&mut self) -> &mut Game {
        self.game.borrow_mut()
    }

    pub fn set_orientation(&mut self, orientation: Player) {
        self.game.borrow_mut().set_orientation(orientation);
    }

    pub fn set_outcome(&mut self, outcome: Option<Outcome>) {
        self.game.borrow_mut().set_outcome(outcome);
    }

    pub fn update_roster(&mut self, update: impl FnOnce(&mut Roster)) {
        self.game.borrow_mut().update_roster(update);
    }

    pub fn update_tags(&mut self, update: impl FnOnce(&mut Vec<tag::Other>)) {
        self.game.borrow_mut().update_tags(update);
    }

    // "reborrowing" clone
    pub fn cursor_mut(&mut self) -> CursorMut<'_> {
        let slot = self.slot;
        let preserve = self.protected().unremovable;
        Cursor { game: self.game.borrow_mut(), slot, preserve }
    }

    pub fn position_mut(&mut self) -> PositionMut<'_> {
        let slot = self.slot;
        let protected = self.protected();
        PositionMut { game: self.game.borrow_mut(), slot, protected }
    }

    pub fn play_mut(&mut self) -> start::Result<MoveMut<'_>> {
        self.position_mut().into_play()
    }

    pub fn at_mut(&mut self, slot: PositionSlot) -> traversal::Result<CursorMut<'_>> {
        if !self.game.borrow().tree.contains(slot) {
            return Err(traversal::Error::Position(slot));
        }

        let preserve = self.protected().unremovable;
        Ok(Cursor { game: self.game.borrow_mut(), slot, preserve })
    }

    fn protected(&self) -> Protected {
        let mut unremovable = self.preserve.clone();
        unremovable.insert(self.slot);
        Protected { unremovable }
    }
}

/// Access API.
impl<'g> CursorRef<'g> {
    pub fn into_game(self) -> &'g Game {
        self.game
    }

    pub fn into_position(self) -> PositionRef<'g> {
        PositionRef { game: self.game, slot: self.slot }
    }

    pub fn into_play(self) -> start::Result<MoveRef<'g>> {
        self.into_position().play()
    }
}

/// Access API.
impl<'g> CursorMut<'g> {
    pub fn into_position_mut(self) -> PositionMut<'g> {
        let slot = self.slot;
        let mut unremovable = self.preserve;
        unremovable.insert(slot);
        PositionMut { game: self.game, slot, protected: Protected { unremovable } }
    }

    pub fn into_play_mut(self) -> start::Result<MoveMut<'g>> {
        self.into_position_mut().into_play()
    }
}

/// Traversal API.
impl<G: Borrow<Game>> Cursor<G> {
    pub fn set(&mut self, slot: PositionSlot) -> traversal::Result<bool> {
        if !self.game.borrow().tree.contains(slot) {
            return Err(traversal::Error::Position(slot));
        }

        let changed = self.slot != slot;
        self.slot = slot;
        Ok(changed)
    }

    pub fn prev(&mut self) -> start::Result<PositionSlot> {
        let slot = self.position().play()?.previous().slot();
        self.slot = slot;
        Ok(slot)
    }

    pub fn mainline_end(&mut self) -> PositionSlot {
        while self.main().is_ok() {}
        self.slot
    }

    pub fn main(&mut self) -> traversal::Result<PositionSlot> {
        self.option(0)
    }

    pub fn option(&mut self, option: impl Choose) -> traversal::Result<PositionSlot> {
        let slot = self.position().option(option)?.position().slot();
        self.slot = slot;
        Ok(slot)
    }
}

/// Options Mutation API
impl<G: BorrowMut<Game>> Cursor<G> {
    pub fn append_moves(
        &mut self,
        moves: impl IntoIterator<Item = chess::Move>,
    ) -> options::Result<()> {
        for play in moves {
            self.option_mut_or_push(play)?;
        }
        Ok(())
    }

    pub fn option_mut_or_push(&mut self, play: chess::Move) -> options::Result<PositionSlot> {
        let slot = self.position_mut().option_mut_or_push(play)?.slot().into();
        self.slot = slot;
        Ok(slot)
    }

    pub fn option_mut_or_insert_before(
        &mut self,
        option: impl Choose,
        play: chess::Move,
    ) -> Result<PositionSlot> {
        let slot = self.position_mut().option_mut_or_insert_before(option, play)?.slot().into();
        self.slot = slot;
        Ok(slot)
    }

    pub fn option_mut_or_insert_after(
        &mut self,
        option: impl Choose,
        play: chess::Move,
    ) -> Result<PositionSlot> {
        let slot = self.position_mut().option_mut_or_insert_after(option, play)?.slot().into();
        self.slot = slot;
        Ok(slot)
    }

    /// Unlike `self.position_mut().into_remove_play()`, this first retreats the cursor so its
    /// former position is no longer protected by the cursor itself.
    pub fn remove_play(&mut self) -> Result<()> {
        let play = self.play()?;
        let removed = self.slot;
        let slot = play.slot();
        self.slot = play.previous().slot();

        if let Err(error) = self.position_mut().remove(slot) {
            self.slot = removed;
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

        assert_eq!(cursor.slot(), START);
        assert!(cursor.prev().is_err());
        assert!(cursor.main().is_err());

        cursor.option_mut_or_push(e4).unwrap();
        let after_e4 = cursor.slot();
        cursor.option_mut_or_push(e5).unwrap();
        let after_e5 = cursor.slot();

        assert_eq!(cursor.prev().unwrap(), after_e4);
        assert_eq!(cursor.main().unwrap(), after_e5);
        assert!(!cursor.set(after_e5).unwrap());
        assert_eq!(**cursor.play().unwrap(), e5);
    }

    #[test]
    fn cursor_removal_protection() {
        let mut game = Game::new(chess::Position::start());
        let e4 = game.start_mut().into_push(chess::Move::normal(Pawn, E2, E4)).unwrap().slot();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .into_position()
            .into_push(chess::Move::normal(Pawn, E7, E5))
            .unwrap()
            .slot();
        let d4 = game.start_mut().into_push(chess::Move::normal(Pawn, D2, D4)).unwrap().slot();

        let mut cursor = Cursor::new(game);
        assert!(cursor.set(e5.into()).unwrap());

        let mut root = cursor.at_mut(START).unwrap();
        let mut start = root.position_mut();
        assert!(matches!(
            start.remove(e4),
            Err(Error::Options(options::Error::Remove(options::Remove::Unremovable(slot))))
                if slot == e5.into()
        ));
        assert!(start.remove(d4).is_ok());

        assert_eq!(cursor.slot(), e5.into());
        assert_eq!(cursor.position().slot(), e5.into());
        assert!(cursor.game().play(e4).is_ok());
        assert!(cursor.game().play(e5).is_ok());
        assert!(cursor.game().play(d4).is_err());
    }

    #[test]
    fn cursor_remove_play() {
        let mut cursor = Cursor::new(Game::new(chess::Position::start()));
        let e4 = cursor.option_mut_or_push(chess::Move::normal(Pawn, E2, E4)).unwrap();
        let e5 = cursor.option_mut_or_push(chess::Move::normal(Pawn, E7, E5)).unwrap();

        assert!(cursor.position_mut().into_remove_play().is_err());
        assert_eq!(cursor.slot(), e5);
        assert!(cursor.remove_play().is_ok());
        assert_eq!(cursor.slot(), e4);
        assert!(cursor.game().position(e5).is_err());
        assert!(cursor.remove_play().is_ok());
        assert_eq!(cursor.slot(), START);
        assert!(matches!(cursor.remove_play(), Err(Error::Start(start::Error))));
    }
}
