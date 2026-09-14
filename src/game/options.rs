use super::{tree::Node, *};

/// Options Mutation API
impl<'g> PositionMut<'g> {
    pub fn push(&mut self, play: chess::Move) -> Result<MoveMut<'_>> {
        let id = self.insert_at(self.options().len(), play)?;
        Ok(MoveMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_push(mut self, play: chess::Move) -> Result<MoveMut<'g>> {
        let id = self.insert_at(self.options().len(), play)?;
        Ok(MoveMut { game: self.game, id, protected: self.protected })
    }

    // the natural same implementation as in into_option_or_push does
    // not work here due to borrow checker limitations - even though
    // it is "obviously" correct.
    pub fn option_mut_or_push(&mut self, play: chess::Move) -> Result<MoveMut<'_>> {
        if let Some(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected.clone() });
        }
        self.push(play)
    }

    pub fn into_option_or_push(self, play: chess::Move) -> Result<MoveMut<'g>> {
        if let Some(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected });
        }
        self.into_push(play)
    }

    pub fn insert_before(&mut self, option: impl Locate, play: chess::Move) -> Result<MoveMut<'_>> {
        let id = self.insert_before_option(option, play)?;
        Ok(MoveMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_insert_before(
        mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> Result<MoveMut<'g>> {
        let id = self.insert_before_option(option, play)?;
        Ok(MoveMut { game: self.game, id, protected: self.protected })
    }

    pub fn option_mut_or_insert_before(
        &mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> Result<MoveMut<'_>> {
        if let Some(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected.clone() });
        }
        self.insert_before(option, play)
    }

    pub fn into_option_or_insert_before(
        self,
        option: impl Locate,
        play: chess::Move,
    ) -> Result<MoveMut<'g>> {
        if let Some(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected });
        }
        self.into_insert_before(option, play)
    }

    pub fn insert_after(&mut self, option: impl Locate, play: chess::Move) -> Result<MoveMut<'_>> {
        let id = self.insert_after_option(option, play)?;
        Ok(MoveMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_insert_after(
        mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> Result<MoveMut<'g>> {
        let id = self.insert_after_option(option, play)?;
        Ok(MoveMut { game: self.game, id, protected: self.protected })
    }

    pub fn option_mut_or_insert_after(
        &mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> Result<MoveMut<'_>> {
        if let Some(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected.clone() });
        }
        self.insert_after(option, play)
    }

    pub fn into_option_or_insert_after(
        self,
        option: impl Locate,
        play: chess::Move,
    ) -> Result<MoveMut<'g>> {
        if let Some(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected });
        }
        self.into_insert_after(option, play)
    }

    pub fn pop(&mut self) -> Result<()> {
        let index = self.options().len().checked_sub(1).ok_or(Error::Missing)?;
        self.remove_index(index)
    }

    pub fn remove(&mut self, option: impl Locate) -> Result<()> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        self.remove_index(index)
    }

    pub fn into_remove_play(self) -> Result<PositionMut<'g>> {
        let play = self.as_ref().play().ok_or(Error::Missing)?;
        let id = play.id();
        let previous = play.previous().id();
        let mut position = PositionMut { game: self.game, id: previous, protected: self.protected };
        position.remove(id)?;
        Ok(position)
    }

    pub fn swap(&mut self, a: impl Locate, b: impl Locate) -> Result<bool> {
        let position = self.as_ref();
        let a = position.index(a).ok_or(Error::Missing)?;
        let b = position.index(b).ok_or(Error::Missing)?;
        Ok(self.swap_index(a, b))
    }

    pub fn raise(&mut self, option: impl Locate) -> Result<bool> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        if index == 0 {
            Ok(false)
        } else {
            self.swap_index(index - 1, index);
            Ok(true)
        }
    }

    pub fn raise_play(&mut self) -> Option<bool> {
        let mut play = self.play_mut()?;
        let id = play.id();
        play.previous_mut().raise(id).ok()
    }

    pub fn lower(&mut self, option: impl Locate) -> Result<bool> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        if index + 1 == self.options().len() {
            Ok(false)
        } else {
            self.swap_index(index, index + 1);
            Ok(true)
        }
    }

    pub fn lower_play(&mut self) -> Option<bool> {
        let mut play = self.play_mut()?;
        let id = play.id();
        play.previous_mut().lower(id).ok()
    }

    pub fn promote(&mut self, option: impl Locate) -> Result<bool> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        if index == 0 {
            return Ok(false);
        }

        let id = self.position_mut().options.remove(index);
        self.position_mut().options.insert(0, id);
        Ok(true)
    }

    pub fn promote_play(&mut self) -> Option<bool> {
        let mut play = self.play_mut()?;
        let id = play.id();
        play.previous_mut().promote(id).ok()
    }

    pub fn demote(&mut self, option: impl Locate) -> Result<bool> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        if index + 1 == self.options().len() {
            return Ok(false);
        }

        let id = self.position_mut().options.remove(index);
        self.position_mut().options.push(id);
        Ok(true)
    }

    pub fn demote_play(&mut self) -> Option<bool> {
        let mut play = self.play_mut()?;
        let id = play.id();
        play.previous_mut().demote(id).ok()
    }

    fn insert_before_option(&mut self, option: impl Locate, play: chess::Move) -> Result<MoveId> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        self.insert_at(index, play)
    }

    fn insert_after_option(&mut self, option: impl Locate, play: chess::Move) -> Result<MoveId> {
        let index = self.as_ref().index(option).ok_or(Error::Missing)?;
        self.insert_at(index + 1, play)
    }

    fn insert_at(&mut self, index: usize, play: chess::Move) -> Result<MoveId> {
        let node = {
            let position = self.as_ref();
            if let Some(index) = position.index(play) {
                return Err(Error::Duplicate(index));
            }
            if !position.legal().contains(&play) {
                return Err(Error::Illegal);
            }

            let play = Move {
                previous: self.id,
                play,
                short: Short::new(position.legal(), play),
                public: MovePublic::default(),
            };
            let position = position.position().apply_unchecked(*play);
            Node { position: Position::new(position), play: Some(play) }
        };

        let id = self.game.tree.insert(node);
        self.position_mut().options.insert(index, id);
        Ok(id)
    }

    fn remove_index(&mut self, index: usize) -> Result<()> {
        let id = self.position().options.get(index).copied().ok_or(Error::Missing)?;
        if let Some(id) =
            self.protected.unremovable.iter().copied().find(|at| self.game.order(*at) >= id.into())
        {
            return Err(Error::Unremovable(id));
        }

        self.position_mut().options.remove(index);
        let removed = self.game.tree.remove(id);
        debug_assert!(removed);
        Ok(())
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

#[cfg(test)]
mod tests {
    use crate::{Role::Pawn, Square::*};

    use super::*;

    #[test]
    fn option_or_insert() {
        let mut game = Game::new(chess::Position::start());
        let e4 = chess::Move::normal(Pawn, E2, E4);
        let d4 = chess::Move::normal(Pawn, D2, D4);
        let c4 = chess::Move::normal(Pawn, C2, C4);
        let f4 = chess::Move::normal(Pawn, F2, F4);

        let e4_id = game.start_mut().push(e4).unwrap().id();
        let d4_id = game.start_mut().option_mut_or_push(d4).unwrap().id();
        let existing = game.start_mut().option_mut_or_insert_after(d4_id, e4).unwrap().id();
        assert_eq!(existing, e4_id);
        assert_eq!(game.start().iter().map(|play| play.id()).collect::<Vec<_>>(), [e4_id, d4_id]);

        let c4_id = game.start_mut().option_mut_or_insert_before(d4_id, c4).unwrap().id();
        let f4_id = game.start_mut().into_option_or_insert_after(c4_id, f4).unwrap().id();
        assert_eq!(
            game.start().iter().map(|play| play.id()).collect::<Vec<_>>(),
            [e4_id, c4_id, f4_id, d4_id]
        );

        let existing = game.start_mut().into_option_or_push(e4).unwrap().id();
        assert_eq!(existing, e4_id);
    }

    #[test]
    fn remove_play() {
        let mut game = Game::new(chess::Position::start());
        let e4 = game.start_mut().push(chess::Move::normal(Pawn, E2, E4)).unwrap().id();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .into_position()
            .into_push(chess::Move::normal(Pawn, E7, E5))
            .unwrap()
            .id();

        let previous = game.position_mut(e5.into()).unwrap().into_remove_play().unwrap();
        assert_eq!(previous.id(), e4.into());
        assert!(!previous.as_ref().has_options());
        assert!(previous.game().play(e5).is_none());
    }
}
