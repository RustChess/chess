use super::*;

/// Cursor over the main line of a linear game.
#[derive(Clone, PartialEq)]
pub struct Cursor {
    game: Game,
    node: Node,
}

pub struct Mainline<'a> {
    game: &'a Game,
    node: Node,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Game(#[from] super::Error),
}

impl Cursor {
    pub fn new(game: Game) -> Self {
        Self { game, node: Node::Start }
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

    pub fn node(&self) -> Node {
        self.node
    }

    pub fn state(&self) -> &State {
        self.game.state(self.node)
    }

    /// Sets whether the options at `node` are expanded.
    #[must_use]
    pub fn set_expanded(&mut self, node: Node, expanded: bool) -> bool {
        let Some(mut options) = self.game.options_mut(node) else {
            return false;
        };
        options.set_expanded(expanded);
        true
    }

    #[must_use]
    pub fn set(&mut self, node: Node) -> bool {
        if !self.game.contains(node) {
            return false;
        }

        self.node = node;
        true
    }

    pub fn play(&self) -> Option<PlayRef<'_>> {
        match self.node {
            Node::Start => None,
            Node::Play(slot) => self.game.play(slot),
        }
    }

    pub fn play_mut(&mut self) -> Option<PlayMut<'_>> {
        match self.node {
            Node::Start => None,
            Node::Play(slot) => self.game.play_mut(slot),
        }
    }

    pub fn set_comment(&mut self, comment: Option<Text>) -> Option<bool> {
        match self.node {
            Node::Start => {
                if self.game.intro == comment {
                    Some(false)
                } else {
                    self.game.intro = comment;
                    Some(true)
                }
            }
            Node::Play(slot) => {
                let mut play = self.game.play_mut(slot)?;
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
        self.game.state_mut(self.node).set_evaluation(evaluation)
    }

    pub fn update_evaluation(&mut self, evaluation: Evaluation) -> bool {
        self.game.state_mut(self.node).update_evaluation(evaluation)
    }

    pub fn options(&self) -> OptionsRef<'_> {
        self.game.options_ref(self.node)
    }

    pub fn previous(&self) -> Node {
        match self.node {
            Node::Start => Node::Start,
            Node::Play(slot) => self
                .game
                .tree
                .play(slot)
                .expect("cursor slot must reference an existing play")
                .previous(),
        }
    }

    pub fn next(&self) -> Option<Node> {
        self.game.tree.options(self.node).first().copied().map(Node::Play)
    }

    #[must_use]
    pub fn back(&mut self) -> bool {
        let previous = self.previous();
        if self.node == previous {
            false
        } else {
            self.node = previous;
            true
        }
    }

    #[must_use]
    pub fn forward(&mut self) -> bool {
        let Some(next) = self.next() else {
            return false;
        };
        self.node = next;
        true
    }

    pub fn start(&mut self) {
        self.node = Node::Start;
    }

    pub fn end(&mut self) {
        while self.forward() {}
    }
}

impl Cursor {
    pub fn position(&self) -> Position {
        self.game.position(self.node)
    }
}

impl Cursor {
    #[must_use]
    pub fn take_back(&mut self) -> bool {
        let Node::Play(slot) = self.node else {
            return false;
        };
        let previous = self.previous();
        let play = self.game.play(slot).expect("cursor slot must exist").play();

        self.game
            .options_mut(previous)
            .expect("previous options must exist")
            .remove(play)
            .expect("current play must be an option of its predecessor");
        self.node = previous;
        true
    }

    pub fn push(&mut self, play: Move) -> Result<Slot, Error> {
        if let Some(next) = self.options().get(play) {
            let slot = next.slot();
            self.node = Node::Play(slot);
            return Ok(slot);
        }

        let slot =
            self.game.options_mut(self.node).expect("cursor node must exist").push(play)?.slot();
        self.node = Node::Play(slot);
        Ok(slot)
    }

    pub fn insert_after(&mut self, after: Slot, play: Move) -> Result<Slot, Error> {
        if let Some(next) = self.options().get(play) {
            let slot = next.slot();
            self.node = Node::Play(slot);
            return Ok(slot);
        }

        let index = self
            .options()
            .iter()
            .position(|option| option.slot() == after)
            .ok_or(super::Error::Illegal)?
            + 1;
        let slot = self
            .game
            .options_mut(self.node)
            .expect("cursor node must exist")
            .into_insert(index, play)?
            .slot();
        self.node = Node::Play(slot);
        Ok(slot)
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
        Self { game, node: Node::Start }
    }
}

impl<'a> Iterator for Mainline<'a> {
    type Item = &'a Play;

    fn next(&mut self) -> Option<Self::Item> {
        let slot = self.game.tree.options(self.node).first().copied()?;
        self.node = Node::Play(slot);
        Some(self.game.tree.play(slot).expect("mainline slot must exist"))
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
        let slot = cursor.push(play).unwrap();

        assert_eq!(cursor.node(), Node::Play(slot));
        assert!(!cursor.forward());
        assert!(cursor.back());
        assert_eq!(cursor.node(), Node::Start);
        assert!(cursor.forward());
        assert_eq!(cursor.node(), Node::Play(slot));

        cursor.start();
        assert_eq!(cursor.push(play).unwrap(), slot);
        assert_eq!(cursor.node(), Node::Play(slot));

        cursor.start();
        let d4 = cursor.push(crate::Move::normal(Pawn, D2, D4)).unwrap();
        assert_ne!(d4, slot);
        assert_eq!(cursor.game().start_options().len(), 2);

        cursor.start();
        assert_eq!(cursor.push(play).unwrap(), slot);
    }

    #[test]
    fn take_back_current_play() {
        let mut cursor = Game::chess(Position::start()).unwrap().cursor();
        let e4 = cursor.push(crate::Move::normal(Pawn, E2, E4)).unwrap();
        cursor.push(crate::Move::normal(Pawn, E7, E5)).unwrap();

        assert!(cursor.take_back());
        assert_eq!(cursor.node(), Node::Play(e4));
        assert!(cursor.next().is_none());

        assert!(cursor.take_back());
        assert_eq!(cursor.node(), Node::Start);
        assert!(cursor.next().is_none());
        assert!(!cursor.take_back());
    }
}
