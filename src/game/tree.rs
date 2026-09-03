use std::collections::BTreeMap as Map;

use crate::position::{Chess, Unvalidated};

use super::Play;

/// A local index into the game's slot map, which actually stores the moves of the game.
pub type Slot = u32;

#[derive(Clone, PartialEq)]
pub struct Tree<Variant = Chess> {
    next: Slot,
    slots: Map<Slot, Play<Variant>>,
    start: Vec<Slot>,
}

/// A handle to a node in the tree of variations of a game of chess.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub enum Node {
    #[default]
    Start,
    Play(Slot),
}

impl<V> Default for Tree<V> {
    fn default() -> Self {
        Tree::new()
    }
}

impl<V> Tree<V> {
    pub const fn new() -> Self {
        Self { next: 0, slots: Map::new(), start: Vec::new() }
    }

    pub fn contains(&self, slot: Slot) -> bool {
        self.slots.contains_key(&slot)
    }

    pub fn play(&self, slot: Slot) -> Option<&Play<V>> {
        self.slots.get(&slot)
    }

    pub(super) fn play_mut(&mut self, slot: Slot) -> Option<&mut Play<V>> {
        self.slots.get_mut(&slot)
    }

    pub fn slots(&self) -> impl Iterator<Item = Slot> + '_ {
        self.slots.keys().copied()
    }

    pub fn plays(&self) -> impl Iterator<Item = &Play<V>> {
        self.slots.values()
    }

    // unused for now, added for consistency
    #[allow(dead_code)]
    pub(super) fn plays_mut(&mut self) -> impl Iterator<Item = &mut Play<V>> {
        self.slots.values_mut()
    }

    pub fn start(&self) -> &[Slot] {
        &self.start
    }

    pub(super) fn start_mut(&mut self) -> &mut Vec<Slot> {
        &mut self.start
    }

    pub fn options(&self, node: Node) -> &[Slot] {
        match node {
            Node::Start => self.start(),
            Node::Play(slot) => &self.play(slot).expect("slot exists").options,
        }
    }

    pub(super) fn options_mut(&mut self, node: Node) -> &mut Vec<Slot> {
        match node {
            Node::Start => self.start_mut(),
            Node::Play(slot) => &mut self.play_mut(slot).expect("slot exists").options,
        }
    }

    // Non-public, Game controls play coherence and legality.
    pub(super) fn insert(&mut self, f: impl FnOnce(Slot) -> Play<V>) -> Slot {
        let slot = self.next;
        self.next += 1;

        let play = f(slot);
        debug_assert_eq!(slot, play.slot);
        self.slots.insert(slot, play);
        slot
    }

    // Could plausibly be public, but would have to return bool to
    // avoid dangling pointers in the move's follow-on options.
    pub(super) fn remove(&mut self, slot: Slot) -> bool {
        let Some(play) = self.slots.remove(&slot) else { return false };
        for option in &play.options {
            self.remove(*option);
        }
        true
    }
}

impl<V: Copy> Tree<V> {
    pub fn unvalidated(self) -> Tree<Unvalidated> {
        Tree {
            next: self.next,
            slots: self.slots.into_iter().map(|(slot, play)| (slot, play.unvalidated())).collect(),
            start: self.start,
        }
    }
}

impl Node {
    pub fn is_start(&self) -> bool {
        matches!(self, Self::Start)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Node {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Start => Option::<Slot>::None.serialize(serializer),
            Self::Play(slot) => Some(*slot).serialize(serializer),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Node {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match Option::<Slot>::deserialize(deserializer)? {
            Some(slot) => Self::Play(slot),
            None => Self::Start,
        })
    }
}
