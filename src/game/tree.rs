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

    pub(in crate::game) fn branch(&self, id: MoveId) -> patch::Branch {
        let node = self.node(id.into());
        let play = node.play();
        patch::Branch {
            id,
            play: play.play,
            public: patch::BranchPublic {
                play: play.public.clone(),
                position: node.position.public.clone(),
            },
            options: node.position.options.iter().map(|id| self.branch(*id)).collect(),
        }
    }

    fn node(&self, id: PositionId) -> &Node {
        self.nodes.get(&id).expect("position exists")
    }

    fn node_mut(&mut self, id: PositionId) -> &mut Node {
        self.nodes.get_mut(&id).expect("position exists")
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
