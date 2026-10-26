use std::collections::BTreeMap as Map;

use super::*;

/// Storage for the starting position and subsequent moves.
#[derive(Clone, PartialEq)]
pub struct Tree {
    next: MoveSlot,
    // PositionSlot::is_start() iff Node::play.is_none()
    nodes: Map<PositionSlot, Node>,
}

/// A position and the optional move that reached it.
///
/// The Position contains the the Vec<MoveSlot> options, the forward refs
/// The Move contains the previous PositionSlot, the backref
/// The optionality of Node.play is where the "optional move that reached it" is encoded.
#[derive(Clone, PartialEq)]
pub struct Node {
    pub position: Position,
    pub play: Option<Move>,
}

impl Tree {
    pub fn new(position: chess::Position, next: MoveSlot) -> Self {
        let start = Node { position: Position::new(position), play: None };
        Self { next, nodes: Map::from([(START, start)]) }
    }

    pub(in crate::game) const fn next(&self) -> MoveSlot {
        self.next
    }

    pub fn contains(&self, slot: PositionSlot) -> bool {
        self.nodes.contains_key(&slot)
    }

    fn node(&self, slot: PositionSlot) -> &Node {
        self.nodes.get(&slot).expect("position exists")
    }

    fn node_mut(&mut self, slot: PositionSlot) -> &mut Node {
        self.nodes.get_mut(&slot).expect("position exists")
    }

    pub(in crate::game) fn less_equal(
        &self,
        ancestor: PositionSlot,
        mut child: PositionSlot,
    ) -> bool {
        while ancestor != child {
            let PositionSlot::Move(slot) = child else { return false };
            child = self.play(slot).previous;
        }
        true
    }

    pub(in crate::game) fn play(&self, slot: MoveSlot) -> &Move {
        self.node(slot.into()).play()
    }

    pub(in crate::game) fn play_mut(&mut self, slot: MoveSlot) -> &mut Move {
        self.node_mut(slot.into()).play_mut()
    }

    pub(in crate::game) fn position(&self, slot: PositionSlot) -> &Position {
        self.node(slot).position()
    }

    pub(in crate::game) fn position_mut(&mut self, slot: PositionSlot) -> &mut Position {
        self.node_mut(slot).position_mut()
    }

    pub(in crate::game) fn clear_analyses(&mut self) {
        for node in self.nodes.values_mut() {
            node.position.analysis = None;
        }
    }

    pub(in crate::game) fn insert(&mut self, node: Node) -> Option<MoveSlot> {
        let slot = self.next;
        self.insert_at(slot, node).then_some(slot)
    }

    pub(in crate::game) fn insert_at(&mut self, slot: MoveSlot, node: Node) -> bool {
        let Some(next) = slot.next() else { return false };
        if self.nodes.contains_key(&slot.into()) {
            return false;
        }

        self.next = self.next.max(next);
        self.nodes.insert(slot.into(), node);
        true
    }

    pub(in crate::game) fn remove(&mut self, slot: MoveSlot) -> bool {
        if !self.remove_branch(slot) {
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

    fn remove_branch(&mut self, slot: MoveSlot) -> bool {
        let Some(node) = self.nodes.remove(&slot.into()) else { return false };
        for option in node.position.options {
            self.remove_branch(option);
        }
        true
    }

    pub(in crate::game) fn branch(&self, slot: MoveSlot) -> change::Branch {
        let node = self.node(slot.into());
        let play = node.play();
        change::Branch {
            slot,
            play: play.play,
            public: change::BranchPublic {
                play: play.public.clone(),
                position: node.position.public.clone(),
            },
            options: node.position.options.iter().map(|slot| self.branch(*slot)).collect(),
        }
    }

    /// Reassigns every move slot according to a reslot produced from this tree's current state.
    pub(in crate::game) fn reslot(&mut self, reslot: &Reslot) {
        let before = &mut self.nodes;
        let mut after = Map::new();

        // handle start position options
        let mut start = before.remove(&START).expect("start position exists");
        reslot.options(&mut start.position);
        after.insert(START, start);

        // handle non-start positions
        for (old, new) in reslot.iter() {
            // take the node
            let mut node = before.remove(&old.into()).expect("move exists");
            let play = node.play.as_mut().expect("play exists");
            // update its backref
            play.previous = reslot.position(play.previous).expect("complete reslot");
            // update its forward refs
            reslot.options(&mut node.position);
            // insert it at its new slot
            after.insert(new.into(), node);
        }

        debug_assert!(before.is_empty());
        // last_key_value is the maximum of the new slots
        self.next = match after.last_key_value().expect("start position exists").0 {
            PositionSlot::Start => MoveSlot::MIN,
            PositionSlot::Move(slot) => slot.next().expect("game tree fits in move slots"),
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
    fn reslot_in_pgn_layout() {
        let mut game = Game::new(ChessPosition::start());
        let e4 = game.start_mut().push(ChessMove::normal(Pawn, E2, E4)).unwrap().slot();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(ChessMove::normal(Pawn, E7, E5))
            .unwrap()
            .slot();
        let d4 = game.start_mut().push(ChessMove::normal(Pawn, D2, D4)).unwrap().slot();
        let c4 = game.start_mut().push(ChessMove::normal(Pawn, C2, C4)).unwrap().slot();
        let pgn = Pgn::from(game.clone());

        let mapping = game.reslot(Layout::Pgn);

        assert_eq!(mapping.play(e4), Some(MoveSlot(NonZeroU32::new(1).unwrap())));
        assert_eq!(mapping.play(d4), Some(MoveSlot(NonZeroU32::new(2).unwrap())));
        assert_eq!(mapping.play(c4), Some(MoveSlot(NonZeroU32::new(3).unwrap())));
        assert_eq!(mapping.play(e5), Some(MoveSlot(NonZeroU32::new(4).unwrap())));
        assert_eq!(
            game.start().options(),
            &[
                MoveSlot(NonZeroU32::new(1).unwrap()),
                MoveSlot(NonZeroU32::new(2).unwrap()),
                MoveSlot(NonZeroU32::new(3).unwrap()),
            ]
        );
        assert_eq!(
            game.play(MoveSlot(NonZeroU32::new(1).unwrap())).unwrap().position().options(),
            &[MoveSlot(NonZeroU32::new(4).unwrap())]
        );
        assert_eq!(
            game.play(MoveSlot(NonZeroU32::new(4).unwrap())).unwrap().previous().slot(),
            MoveSlot(NonZeroU32::new(1).unwrap()).into()
        );
        assert_eq!(game.tree.next(), MoveSlot(NonZeroU32::new(5).unwrap()));
        assert_eq!(Pgn::from(game), pgn);
    }

    #[test]
    fn reslot_is_idempotent() {
        let mut game = Game::new(ChessPosition::start());
        game.start_mut().push(ChessMove::normal(Pawn, E2, E4)).unwrap();
        game.start_mut().push(ChessMove::normal(Pawn, D2, D4)).unwrap();

        game.reslot(Layout::Pgn);
        let reslotted = game.clone();
        let mapping = game.reslot(Layout::Pgn);

        assert!(game == reslotted);
        assert!(mapping.iter().all(|(previous, slot)| previous == slot));
    }
}
