use std::collections::BTreeMap as Map;

use super::*;

/// Storage for the starting position and subsequent moves.
#[derive(Clone, PartialEq)]
pub struct Tree {
    next: MoveId,
    // PositionId::is_start() iff Node::play.is_none()
    nodes: Map<PositionId, Node>,
}

/// A position and the optional move that reached it.
///
/// The Position contains the the Vec<MoveId> options, the forward refs
/// The Move contains the previous PositionId, the backref
/// The optionality of Node.play is where the "optional move that reached it" is encoded.
#[derive(Clone, PartialEq)]
pub struct Node {
    pub position: Position,
    pub play: Option<Move>,
}

impl Tree {
    pub fn new(position: chess::Position, next: MoveId) -> Self {
        let start = Node { position: Position::new(position), play: None };
        Self { next, nodes: Map::from([(START, start)]) }
    }

    pub(in crate::game) const fn next(&self) -> MoveId {
        self.next
    }

    pub fn contains(&self, id: PositionId) -> bool {
        self.nodes.contains_key(&id)
    }

    fn node(&self, id: PositionId) -> &Node {
        self.nodes.get(&id).expect("position exists")
    }

    fn node_mut(&mut self, id: PositionId) -> &mut Node {
        self.nodes.get_mut(&id).expect("position exists")
    }

    pub(in crate::game) fn less_equal(&self, ancestor: PositionId, mut child: PositionId) -> bool {
        while ancestor != child {
            let PositionId::Move(id) = child else { return false };
            child = self.play(id).previous;
        }
        true
    }

    pub(in crate::game) fn play(&self, id: MoveId) -> &Move {
        self.node(id.into()).play()
    }

    pub(in crate::game) fn play_mut(&mut self, id: MoveId) -> &mut Move {
        self.node_mut(id.into()).play_mut()
    }

    pub(in crate::game) fn position(&self, id: PositionId) -> &Position {
        self.node(id).position()
    }

    pub(in crate::game) fn position_mut(&mut self, id: PositionId) -> &mut Position {
        self.node_mut(id).position_mut()
    }

    pub(in crate::game) fn insert(&mut self, node: Node) -> Option<MoveId> {
        let id = self.next;
        self.insert_at(id, node).then_some(id)
    }

    pub(in crate::game) fn insert_at(&mut self, id: MoveId, node: Node) -> bool {
        let Some(next) = id.next() else { return false };
        if self.nodes.contains_key(&id.into()) {
            return false;
        }

        self.next = self.next.max(next);
        self.nodes.insert(id.into(), node);
        true
    }

    pub(in crate::game) fn remove(&mut self, id: MoveId) -> bool {
        if !self.remove_branch(id) {
            return false;
        }

        while let Some(previous) = self.next.prev() {
            if self.nodes.contains_key(&previous.into()) {
                break;
            }
            self.next = previous;
        }
        true
    }

    fn remove_branch(&mut self, id: MoveId) -> bool {
        let Some(node) = self.nodes.remove(&id.into()) else { return false };
        for option in node.position.options {
            self.remove_branch(option);
        }
        true
    }

    pub(in crate::game) fn branch(&self, id: MoveId) -> change::Branch {
        let node = self.node(id.into());
        let play = node.play();
        change::Branch {
            id,
            play: play.play,
            public: change::BranchPublic {
                play: play.public.clone(),
                position: node.position.public.clone(),
            },
            options: node.position.options.iter().map(|id| self.branch(*id)).collect(),
        }
    }

    /// Reassigns every move ID according to a compaction produced from this tree's current state.
    pub(in crate::game) fn compact(&mut self, compact: &Compact) {
        let before = &mut self.nodes;
        let mut after = Map::new();

        // handle start position options
        let mut start = before.remove(&START).expect("start position exists");
        compact.options(&mut start.position);
        after.insert(START, start);

        // handle non-start positions
        for (old, new) in compact.iter() {
            // take the node
            let mut node = before.remove(&old.into()).expect("move exists");
            let play = node.play.as_mut().expect("play exists");
            // update its backref
            play.previous = compact.position(play.previous).expect("complete compaction");
            // update its forward refs
            compact.options(&mut node.position);
            // insert it at its new ID
            after.insert(new.into(), node);
        }

        debug_assert!(before.is_empty());
        // last_key_value is the maximum of the compacted IDs
        self.next = match after.last_key_value().expect("start position exists").0 {
            PositionId::Start => MoveId::MIN,
            PositionId::Move(id) => id.next().expect("game tree fits in move IDs"),
        };
        self.nodes = after;
    }
}

impl Node {
    const fn play(&self) -> &Move {
        self.play.as_ref().expect("play exists")
    }

    const fn play_mut(&mut self) -> &mut Move {
        self.play.as_mut().expect("play exists")
    }

    const fn position(&self) -> &Position {
        &self.position
    }

    const fn position_mut(&mut self) -> &mut Position {
        &mut self.position
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Move as ChessMove, Position as ChessPosition,
        board::Role::Pawn,
        formats::Pgn,
        square::Square::{C2, C4, D2, D4, E2, E4, E5, E7},
    };

    use super::*;

    #[test]
    fn compact_in_pgn_layout() {
        let mut game = Game::new(ChessPosition::start());
        let e4 = game.start_mut().push(ChessMove::normal(Pawn, E2, E4)).unwrap().id();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(ChessMove::normal(Pawn, E7, E5))
            .unwrap()
            .id();
        let d4 = game.start_mut().push(ChessMove::normal(Pawn, D2, D4)).unwrap().id();
        let c4 = game.start_mut().push(ChessMove::normal(Pawn, C2, C4)).unwrap().id();
        let pgn = Pgn::from(game.clone());

        let mapping = game.compact(Layout::Pgn);

        assert_eq!(mapping.play(e4), Some(MoveId(NonZeroU32::new(1).unwrap())));
        assert_eq!(mapping.play(d4), Some(MoveId(NonZeroU32::new(2).unwrap())));
        assert_eq!(mapping.play(c4), Some(MoveId(NonZeroU32::new(3).unwrap())));
        assert_eq!(mapping.play(e5), Some(MoveId(NonZeroU32::new(4).unwrap())));
        assert_eq!(
            game.start().options(),
            &[
                MoveId(NonZeroU32::new(1).unwrap()),
                MoveId(NonZeroU32::new(2).unwrap()),
                MoveId(NonZeroU32::new(3).unwrap()),
            ]
        );
        assert_eq!(
            game.play(MoveId(NonZeroU32::new(1).unwrap())).unwrap().position().options(),
            &[MoveId(NonZeroU32::new(4).unwrap())]
        );
        assert_eq!(
            game.play(MoveId(NonZeroU32::new(4).unwrap())).unwrap().previous().id(),
            MoveId(NonZeroU32::new(1).unwrap()).into()
        );
        assert_eq!(game.tree.next(), MoveId(NonZeroU32::new(5).unwrap()));
        assert_eq!(Pgn::from(game), pgn);
    }

    #[test]
    fn compact_is_idempotent() {
        let mut game = Game::new(ChessPosition::start());
        game.start_mut().push(ChessMove::normal(Pawn, E2, E4)).unwrap();
        game.start_mut().push(ChessMove::normal(Pawn, D2, D4)).unwrap();

        game.compact(Layout::Pgn);
        let compact = game.clone();
        let mapping = game.compact(Layout::Pgn);

        assert!(game == compact);
        assert!(mapping.iter().all(|(previous, id)| previous == id));
    }
}
