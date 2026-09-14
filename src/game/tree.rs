use std::collections::BTreeMap as Map;

use super::*;

/// Storage for the starting position and subsequent plays.
#[derive(Clone, PartialEq)]
pub struct Tree {
    next: PlayId,
    // PositionId::is_start() iff Node::play.is_none()
    nodes: Map<PositionId, Node>,
}

/// A position and the optional play that reached it.
#[derive(Clone, PartialEq)]
pub struct Node {
    pub position: Position,
    pub play: Option<Play>,
}

impl Tree {
    pub fn new(position: chess::Position) -> Self {
        let start = Node { position: Position::new(position), play: None };
        Self { next: PlayId::default(), nodes: Map::from([(START, start)]) }
    }

    pub fn contains(&self, id: PositionId) -> bool {
        self.nodes.contains_key(&id)
    }

    pub(in crate::game) fn less_equal(&self, ancestor: PositionId, mut child: PositionId) -> bool {
        while ancestor != child {
            let PositionId::Play(id) = child else { return false };
            child = self.play(id).previous;
        }
        true
    }

    pub(in crate::game) fn play(&self, id: PlayId) -> &Play {
        self.node(id.into()).play()
    }

    pub(in crate::game) fn play_mut(&mut self, id: PlayId) -> &mut Play {
        self.node_mut(id.into()).play_mut()
    }

    pub(in crate::game) fn position(&self, id: PositionId) -> &Position {
        self.node(id).position()
    }

    pub(in crate::game) fn position_mut(&mut self, id: PositionId) -> &mut Position {
        self.node_mut(id).position_mut()
    }

    pub(in crate::game) fn insert(&mut self, node: Node) -> PlayId {
        let id = self.next;
        self.next.0 += 1;
        self.nodes.insert(id.into(), node);
        id
    }

    pub(in crate::game) fn remove(&mut self, id: PlayId) -> bool {
        let Some(node) = self.nodes.remove(&id.into()) else { return false };
        for option in node.position.options {
            self.remove(option);
        }
        true
    }

    fn node(&self, id: PositionId) -> &Node {
        self.nodes.get(&id).expect("position exists")
    }

    fn node_mut(&mut self, id: PositionId) -> &mut Node {
        self.nodes.get_mut(&id).expect("position exists")
    }
}

impl Node {
    const fn play(&self) -> &Play {
        self.play.as_ref().expect("play exists")
    }

    const fn play_mut(&mut self) -> &mut Play {
        self.play.as_mut().expect("play exists")
    }

    const fn position(&self) -> &Position {
        &self.position
    }

    const fn position_mut(&mut self) -> &mut Position {
        &mut self.position
    }
}
