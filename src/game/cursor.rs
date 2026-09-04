use super::*;

/// Cursor over the main line of a linear game.
#[derive(Clone, PartialEq)]
pub struct Cursor<Variant = Chess> {
    game: Game<Variant>,
    node: Node,
}

pub struct Mainline<'a, Variant = Chess> {
    game: &'a Game<Variant>,
    node: Node,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Game(#[from] super::Error),
    #[error("position already has main play {0:?}")]
    Nonlinear(Node),
}

impl<V> Cursor<V> {
    pub fn new(game: Game<V>) -> Self {
        Self { game, node: Node::Start }
    }

    pub fn into_inner(self) -> Game<V> {
        self.game
    }

    pub fn game(&self) -> &Game<V> {
        &self.game
    }

    pub fn into_game(self) -> Game<V> {
        self.game
    }

    pub fn node(&self) -> Node {
        self.node
    }

    pub fn state(&self) -> &State<V> {
        self.game.state(self.node)
    }

    #[must_use]
    pub fn set(&mut self, node: Node) -> bool {
        if !self.game.contains(node) {
            return false;
        }

        self.node = node;
        true
    }

    pub fn play(&self) -> Option<PlayRef<'_, V>> {
        match self.node {
            Node::Start => None,
            Node::Play(slot) => self.game.play(slot),
        }
    }

    pub fn options(&self) -> OptionsRef<'_, V> {
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

impl<V> Cursor<V> {
    pub fn position(&self) -> Position<V> {
        self.game.position(self.node)
    }
}

impl<V: Variant> Cursor<V> {
    #[must_use]
    pub fn take_back(&mut self) -> bool {
        let Node::Play(slot) = self.node else {
            return false;
        };
        let previous = self.previous();
        let play = self.game.play(slot).expect("cursor slot must exist").play();

        let _ = self.game.options_mut(previous).expect("previous options must exist").remove(play);
        self.node = previous;
        true
    }

    pub fn push(&mut self, play: Move) -> Result<Slot, Error> {
        if let Some(next) = self.next() {
            return Err(Error::Nonlinear(next));
        }

        let slot =
            self.game.options_mut(self.node).expect("cursor node must exist").push(play)?.slot();
        self.node = Node::Play(slot);
        Ok(slot)
    }
}

impl<'a, V> Mainline<'a, V> {
    pub fn new(game: &'a Game<V>) -> Self {
        Self { game, node: Node::Start }
    }
}

impl<'a, V> Iterator for Mainline<'a, V> {
    type Item = &'a Play<V>;

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
    fn walks_main_line() {
        let mut cursor = Game::new(Position::start()).cursor();
        let play = cursor.position().legal_moves()[0];
        let slot = cursor.push(play).unwrap();

        assert_eq!(cursor.node(), Node::Play(slot));
        assert!(!cursor.forward());
        assert!(cursor.back());
        assert_eq!(cursor.node(), Node::Start);
        assert!(cursor.forward());
        assert_eq!(cursor.node(), Node::Play(slot));

        cursor.start();
        assert!(matches!(
            cursor.push(cursor.position().legal_moves()[0]),
            Err(Error::Nonlinear(existing)) if existing == Node::Play(slot)
        ));
    }

    #[test]
    fn takes_back_current_play() {
        let mut cursor = Game::new(Position::start()).cursor();
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
