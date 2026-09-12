use super::*;

/// Cursor over the main line of a linear game.
#[derive(Clone, PartialEq)]
pub struct Cursor {
    game: Game,
    id: PositionId,
}

pub struct Mainline<'a> {
    game: &'a Game,
    id: PositionId,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Game(#[from] super::Error),
}

impl Cursor {
    pub fn new(game: Game) -> Self {
        Self { game, id: PositionId::Start }
    }

    pub fn into_inner(self) -> Game {
        self.game
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn set_orientation(&mut self, orientation: Player) {
        self.game.orientation = orientation;
    }

    pub fn into_game(self) -> Game {
        self.game
    }

    pub fn position_id(&self) -> PositionId {
        self.id
    }

    pub fn state(&self) -> &State {
        self.game.state(self.id)
    }

    /// Sets whether the options at `id` are expanded.
    #[must_use]
    pub fn set_expanded(&mut self, id: PositionId, expanded: bool) -> bool {
        let Some(mut options) = self.game.options_mut(id) else {
            return false;
        };
        options.set_expanded(expanded);
        true
    }

    #[must_use]
    pub fn set(&mut self, id: PositionId) -> bool {
        if !self.game.contains(id) {
            return false;
        }

        self.id = id;
        true
    }

    pub fn play(&self) -> Option<PlayRef<'_>> {
        match self.id {
            PositionId::Start => None,
            PositionId::Play(id) => self.game.play(id),
        }
    }

    pub fn play_mut(&mut self) -> Option<PlayMut<'_>> {
        match self.id {
            PositionId::Start => None,
            PositionId::Play(id) => self.game.play_mut(id),
        }
    }

    pub fn set_comment(&mut self, comment: Option<Text>) -> Option<bool> {
        match self.id {
            PositionId::Start => {
                if self.game.intro == comment {
                    Some(false)
                } else {
                    self.game.intro = comment;
                    Some(true)
                }
            }
            PositionId::Play(id) => {
                let mut play = self.game.play_mut(id)?;
                if play.meta.comment == comment {
                    Some(false)
                } else {
                    play.meta.comment = comment;
                    Some(true)
                }
            }
        }
    }

    pub fn set_evaluation(&mut self, evaluation: Option<Evaluation>) -> bool {
        self.game.state_mut(self.id).set_evaluation(evaluation)
    }

    pub fn update_evaluation(&mut self, evaluation: Evaluation) -> bool {
        self.game.state_mut(self.id).update_evaluation(evaluation)
    }

    pub fn options(&self) -> OptionsRef<'_> {
        self.game.options_ref(self.id)
    }

    pub fn previous(&self) -> PositionId {
        match self.id {
            PositionId::Start => PositionId::Start,
            PositionId::Play(id) => self
                .game
                .tree
                .play(id)
                .expect("cursor ID must reference an existing play")
                .previous(),
        }
    }

    pub fn next(&self) -> Option<PositionId> {
        self.game.tree.options(self.id).first().copied().map(PositionId::Play)
    }

    #[must_use]
    pub fn back(&mut self) -> bool {
        let previous = self.previous();
        if self.id == previous {
            false
        } else {
            self.id = previous;
            true
        }
    }

    #[must_use]
    pub fn forward(&mut self) -> bool {
        let Some(next) = self.next() else {
            return false;
        };
        self.id = next;
        true
    }

    pub fn start(&mut self) {
        self.id = PositionId::Start;
    }

    pub fn end(&mut self) {
        while self.forward() {}
    }
}

impl Cursor {
    pub fn position(&self) -> Position {
        self.game.position(self.id)
    }
}

impl Cursor {
    #[must_use]
    pub fn take_back(&mut self) -> bool {
        let PositionId::Play(id) = self.id else {
            return false;
        };
        let previous = self.previous();
        let play = self.game.play(id).expect("cursor play must exist").play();

        self.game
            .options_mut(previous)
            .expect("previous options must exist")
            .remove(play)
            .expect("current play must be an option of its predecessor");
        self.id = previous;
        true
    }

    pub fn push(&mut self, play: Move) -> Result<PlayId, Error> {
        if let Some(next) = self.options().get(play) {
            let id = next.id();
            self.id = PositionId::Play(id);
            return Ok(id);
        }

        let id =
            self.game.options_mut(self.id).expect("cursor position must exist").push(play)?.id();
        self.id = PositionId::Play(id);
        Ok(id)
    }

    pub fn insert_after(&mut self, after: PlayId, play: Move) -> Result<PlayId, Error> {
        if let Some(next) = self.options().get(play) {
            let id = next.id();
            self.id = PositionId::Play(id);
            return Ok(id);
        }

        let index = self
            .options()
            .iter()
            .position(|option| option.id() == after)
            .ok_or(super::Error::Illegal)?
            + 1;
        let id = self
            .game
            .options_mut(self.id)
            .expect("cursor position must exist")
            .into_insert(index, play)?
            .id();
        self.id = PositionId::Play(id);
        Ok(id)
    }

    #[must_use]
    pub fn raise(&mut self) -> Option<bool> {
        let play = self.play()?;
        let previous = play.previous();
        let play = play.play();
        self.game.options_mut(previous)?.raise(play)
    }

    #[must_use]
    pub fn lower(&mut self) -> Option<bool> {
        let play = self.play()?;
        let previous = play.previous();
        let play = play.play();
        self.game.options_mut(previous)?.lower(play)
    }

    #[must_use]
    pub fn promote(&mut self) -> Option<bool> {
        let play = self.play()?;
        let previous = play.previous();
        let play = play.play();
        self.game.options_mut(previous)?.promote(play)
    }

    #[must_use]
    pub fn demote(&mut self) -> Option<bool> {
        let play = self.play()?;
        let previous = play.previous();
        let play = play.play();
        self.game.options_mut(previous)?.demote(play)
    }
}

impl<'a> Mainline<'a> {
    pub fn new(game: &'a Game) -> Self {
        Self { game, id: PositionId::Start }
    }
}

impl<'a> Iterator for Mainline<'a> {
    type Item = &'a Play;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.game.tree.options(self.id).first().copied()?;
        self.id = PositionId::Play(id);
        Some(self.game.tree.play(id).expect("mainline play must exist"))
    }
}

#[cfg(test)]
mod tests {
    use crate::{Position, board::Role::*, square::Square::*};

    use super::*;

    #[test]
    fn walk_main_line() {
        let mut cursor = Game::chess(Position::start()).unwrap().cursor();
        let play = cursor.position().legal_moves()[0];
        let id = cursor.push(play).unwrap();

        assert_eq!(cursor.position_id(), PositionId::Play(id));
        assert!(!cursor.forward());
        assert!(cursor.back());
        assert_eq!(cursor.position_id(), PositionId::Start);
        assert!(cursor.forward());
        assert_eq!(cursor.position_id(), PositionId::Play(id));

        cursor.start();
        assert_eq!(cursor.push(play).unwrap(), id);
        assert_eq!(cursor.position_id(), PositionId::Play(id));

        cursor.start();
        let d4 = cursor.push(crate::Move::normal(Pawn, D2, D4)).unwrap();
        assert_ne!(d4, id);
        assert_eq!(cursor.game().start_options().len(), 2);

        cursor.start();
        assert_eq!(cursor.push(play).unwrap(), id);
    }

    #[test]
    fn take_back_current_play() {
        let mut cursor = Game::chess(Position::start()).unwrap().cursor();
        let e4 = cursor.push(crate::Move::normal(Pawn, E2, E4)).unwrap();
        cursor.push(crate::Move::normal(Pawn, E7, E5)).unwrap();

        assert!(cursor.take_back());
        assert_eq!(cursor.position_id(), PositionId::Play(e4));
        assert!(cursor.next().is_none());

        assert!(cursor.take_back());
        assert_eq!(cursor.position_id(), PositionId::Start);
        assert!(cursor.next().is_none());
        assert!(!cursor.take_back());
    }
}
