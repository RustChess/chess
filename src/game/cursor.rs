use super::*;

/// Cursor over the main line of a linear game.
#[derive(Clone)]
pub struct Cursor {
    game: Game,
    at: Option<Id>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Game(#[from] super::Error),
    #[error("position already has main play {0}")]
    Nonlinear(Id),
}

impl Cursor {
    pub fn new(game: Game) -> Self {
        Self { game, at: None }
    }

    pub fn game(&self) -> &Game {
        &self.game
    }

    pub fn into_game(self) -> Game {
        self.game
    }

    pub fn at(&self) -> Option<&Id> {
        self.at.as_ref()
    }

    pub fn play(&self) -> Option<PlayRef<'_>> {
        self.at.clone().and_then(|id| self.game.play(id))
    }

    pub fn position(&self) -> Position {
        self.game.previous_state(self.at.as_ref()).position
    }

    pub fn lines(&self) -> LinesRef<'_> {
        self.game.lines_ref(self.at.clone())
    }

    pub fn previous(&self) -> Option<&Id> {
        let at = self.at.as_ref()?;
        self.game.slots.get(at).expect("cursor id must reference an existing play").previous()
    }

    pub fn next(&self) -> Option<&Id> {
        self.game.lines_slice(self.at.as_ref()).first()
    }

    #[must_use]
    pub fn back(&mut self) -> bool {
        match self.previous().cloned() {
            Some(previous) => {
                self.at = Some(previous);
                true
            }
            None if self.at.is_some() => {
                self.at = None;
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn forward(&mut self) -> bool {
        let Some(next) = self.next().cloned() else {
            return false;
        };
        self.at = Some(next);
        true
    }

    pub fn start(&mut self) {
        self.at = None;
    }

    pub fn end(&mut self) {
        while self.forward() {}
    }

    pub fn push(&mut self, play: Move) -> Result<Id, Error> {
        if let Some(next) = self.next() {
            return Err(Error::Nonlinear(next.clone()));
        }

        let id = self.game.push_line(self.at.clone(), play)?;
        self.at = Some(id.clone());
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use crate::Position;

    use super::*;

    #[test]
    fn walks_main_line() {
        let mut cursor = Game::new(Position::standard()).cursor();
        let play = cursor.position().legal_moves()[0];
        let id = cursor.push(play).unwrap();

        assert_eq!(cursor.at(), Some(&id));
        assert!(!cursor.forward());
        assert!(cursor.back());
        assert_eq!(cursor.at(), None);
        assert!(cursor.forward());
        assert_eq!(cursor.at(), Some(&id));

        cursor.start();
        assert!(matches!(
            cursor.push(cursor.position().legal_moves()[0]),
            Err(Error::Nonlinear(existing)) if existing == id
        ));
    }
}
