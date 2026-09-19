use super::{self as game, traversal::Locate, tree::Node, *};

/// An error from mutating the options in a game tree.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Insert(#[from] Insert),
    #[error(transparent)]
    Remove(#[from] Remove),
}

/// An error from inserting an option.
#[derive(Debug, thiserror::Error)]
pub enum Insert {
    #[error("move already exists at index {0}")]
    Duplicate(usize),
    #[error("illegal move")]
    Illegal,
    #[error("move ID space exhausted")]
    Exhausted,
}

/// An error from removing an option.
#[derive(Debug, thiserror::Error)]
pub enum Remove {
    #[error("position {0:?} may not be removed")]
    Unremovable(PositionId),
}

/// A result of mutating the options in a game tree.
pub type Result<T, E = Error> = core::result::Result<T, E>;

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
        if let Ok(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected.clone() });
        }
        self.push(play)
    }

    pub fn into_option_or_push(self, play: chess::Move) -> Result<MoveMut<'g>> {
        if let Ok(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected });
        }
        self.into_push(play)
    }

    pub fn insert_before(
        &mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> game::Result<MoveMut<'_>> {
        let index = self.as_ref().index(option)?;
        let id = self.insert_at(index, play)?;
        Ok(MoveMut { game: self.game, id, protected: self.protected.clone() })
    }

    pub fn into_insert_before(
        mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> game::Result<MoveMut<'g>> {
        let index = self.as_ref().index(option)?;
        let id = self.insert_at(index, play)?;
        Ok(MoveMut { game: self.game, id, protected: self.protected })
    }

    pub fn option_mut_or_insert_before(
        &mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> game::Result<MoveMut<'_>> {
        if let Ok(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected.clone() });
        }
        self.insert_before(option, play)
    }

    pub fn into_option_or_insert_before(
        self,
        option: impl Locate,
        play: chess::Move,
    ) -> game::Result<MoveMut<'g>> {
        if let Ok(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected });
        }
        self.into_insert_before(option, play)
    }

    pub fn insert_after(
        &mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> game::Result<MoveMut<'_>> {
        let index = self.as_ref().index(option)? + 1;
        if index == self.options().len() {
            Ok(self.push(play)?)
        } else {
            self.insert_before(index, play)
        }
    }

    pub fn into_insert_after(
        self,
        option: impl Locate,
        play: chess::Move,
    ) -> game::Result<MoveMut<'g>> {
        let index = self.as_ref().index(option)? + 1;
        if index == self.options().len() {
            Ok(self.into_push(play)?)
        } else {
            self.into_insert_before(index, play)
        }
    }

    pub fn option_mut_or_insert_after(
        &mut self,
        option: impl Locate,
        play: chess::Move,
    ) -> game::Result<MoveMut<'_>> {
        if let Ok(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected.clone() });
        }
        self.insert_after(option, play)
    }

    pub fn into_option_or_insert_after(
        self,
        option: impl Locate,
        play: chess::Move,
    ) -> game::Result<MoveMut<'g>> {
        if let Ok(id) = self.as_ref().option(play).map(|option| option.id()) {
            return Ok(MoveMut { game: self.game, id, protected: self.protected });
        }
        self.into_insert_after(option, play)
    }

    pub fn pop(&mut self) -> game::Result<()> {
        self.remove(self.options().len().saturating_sub(1))
    }

    pub fn remove(&mut self, option: impl Locate) -> game::Result<()> {
        let index = self.as_ref().index(option)?;
        let id = self.position().options[index];
        if let Some(id) =
            self.protected.unremovable.iter().copied().find(|at| self.game.order(*at) >= id.into())
        {
            return Err(Error::from(Remove::Unremovable(id)).into());
        }

        let branch = self.game.undo.on.then(|| self.game.tree.branch(id));
        self.remove_untracked(index)?;
        if let Some(branch) = branch {
            self.game.undo.stack.push(patch::Patch::Insert { position: self.id, index, branch });
        }
        Ok(())
    }

    pub fn into_remove_play(self) -> game::Result<PositionMut<'g>> {
        let play = self.as_ref().play()?;
        let id = play.id();
        let previous = play.previous().id();
        let mut position = PositionMut { game: self.game, id: previous, protected: self.protected };
        position.remove(id)?;
        Ok(position)
    }

    pub fn swap(&mut self, a: impl Locate, b: impl Locate) -> traversal::Result<bool> {
        let (a, b) = {
            let position = self.as_ref();
            let a = position.index(a)?;
            let b = position.index(b)?;
            (a, b)
        };
        let changed = self.swap_untracked(a, b)?;
        if self.game.undo.on && changed {
            self.game.undo.stack.push(patch::Patch::Swap { position: self.id, a, b });
        }
        Ok(changed)
    }

    pub fn raise(&mut self, option: impl Locate) -> traversal::Result<bool> {
        let index = self.as_ref().index(option)?;
        if index == 0 { Ok(false) } else { self.reorder_tracked(index, index - 1) }
    }

    pub fn raise_play(&mut self) -> start::Result<bool> {
        let mut play = self.play_mut()?;
        let id = play.id();
        Ok(play.previous_mut().raise(id).expect("incoming move exists in previous position"))
    }

    pub fn lower(&mut self, option: impl Locate) -> traversal::Result<bool> {
        let index = self.as_ref().index(option)?;
        if index + 1 == self.options().len() {
            Ok(false)
        } else {
            self.reorder_tracked(index, index + 1)
        }
    }

    pub fn lower_play(&mut self) -> start::Result<bool> {
        let mut play = self.play_mut()?;
        let id = play.id();
        Ok(play.previous_mut().lower(id).expect("incoming move exists in previous position"))
    }

    pub fn promote(&mut self, option: impl Locate) -> traversal::Result<bool> {
        let index = self.as_ref().index(option)?;
        if index == 0 {
            return Ok(false);
        }

        self.reorder_tracked(index, 0)
    }

    pub fn promote_play(&mut self) -> start::Result<bool> {
        let mut play = self.play_mut()?;
        let id = play.id();
        Ok(play.previous_mut().promote(id).expect("incoming move exists in previous position"))
    }

    pub fn demote(&mut self, option: impl Locate) -> traversal::Result<bool> {
        let index = self.as_ref().index(option)?;
        if index + 1 == self.options().len() {
            return Ok(false);
        }

        let last = self.options().len() - 1;
        self.reorder_tracked(index, last)
    }

    pub fn demote_play(&mut self) -> start::Result<bool> {
        let mut play = self.play_mut()?;
        let id = play.id();
        Ok(play.previous_mut().demote(id).expect("incoming move exists in previous position"))
    }

    fn insert_at(&mut self, index: usize, play: chess::Move) -> Result<MoveId> {
        let id = self.insert_at_untracked(None, index, play)?;
        if self.game.undo.on {
            self.game.undo.stack.push(patch::Patch::Remove(id));
        }
        Ok(id)
    }

    pub(in crate::game) fn insert_at_untracked(
        &mut self,
        id: Option<MoveId>,
        index: usize,
        play: chess::Move,
    ) -> Result<MoveId> {
        let node = {
            let position = self.as_ref();
            if let Ok(index) = position.index(play) {
                return Err(Insert::Duplicate(index).into());
            }
            if !position.legal().contains(&play) {
                return Err(Insert::Illegal.into());
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

        let id = match id {
            Some(id) => {
                if id.next().is_none() {
                    return Err(Insert::Exhausted.into());
                }
                let inserted = self.game.tree.insert_at(id, node);
                assert!(inserted, "move ID unavailable");
                id
            }
            None => self.game.tree.insert(node).ok_or(Insert::Exhausted)?,
        };
        self.position_mut().options.insert(index, id);
        Ok(id)
    }

    pub(in crate::game) fn remove_untracked(
        &mut self,
        option: impl Locate,
    ) -> traversal::Result<()> {
        let index = self.as_ref().index(option)?;
        let id = self.position().options[index];
        self.position_mut().options.remove(index);
        let removed = self.game.tree.remove(id);
        debug_assert!(removed);
        Ok(())
    }

    pub(in crate::game) fn swap_untracked(
        &mut self,
        a: impl Locate,
        b: impl Locate,
    ) -> traversal::Result<bool> {
        let a = self.as_ref().index(a)?;
        let b = self.as_ref().index(b)?;
        if a == b {
            Ok(false)
        } else {
            self.position_mut().options.swap(a, b);
            Ok(true)
        }
    }

    pub(in crate::game) fn reorder_untracked(
        &mut self,
        from: impl Locate,
        to: impl Locate,
    ) -> traversal::Result<bool> {
        let from = self.as_ref().index(from)?;
        let to = self.as_ref().index(to)?;
        if from == to {
            return Ok(false);
        }

        let play = self.position_mut().options.remove(from);
        self.position_mut().options.insert(to, play);
        Ok(true)
    }

    fn reorder_tracked(&mut self, from: usize, to: usize) -> traversal::Result<bool> {
        let changed = self.reorder_untracked(from, to)?;
        if self.game.undo.on && changed {
            self.game.undo.stack.push(patch::Patch::Reorder {
                position: self.id,
                from: to,
                to: from,
            });
        }
        Ok(changed)
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
        assert!(previous.game().play(e5).is_err());
    }
}
