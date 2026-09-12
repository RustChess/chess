use std::collections::BTreeMap as Map;

use super::{Options, Play};

/// A local index into the game's slot map, which actually stores the moves of the game.
pub type PlayId = u32;

#[derive(Clone, PartialEq)]
pub struct Tree {
    next: PlayId,
    slots: Map<PlayId, Play>,
    start: Options,
}

/// A handle to a node in the tree of variations of a game of chess.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub enum PositionId {
    #[default]
    Start,
    Play(PlayId),
}

impl Default for Tree {
    fn default() -> Self {
        Tree::new()
    }
}

impl Tree {
    pub const fn new() -> Self {
        Self { next: 0, slots: Map::new(), start: Options::new() }
    }

    pub fn contains(&self, id: PlayId) -> bool {
        self.slots.contains_key(&id)
    }

    pub fn play(&self, id: PlayId) -> Option<&Play> {
        self.slots.get(&id)
    }

    pub(super) fn play_mut(&mut self, id: PlayId) -> Option<&mut Play> {
        self.slots.get_mut(&id)
    }

    pub fn slots(&self) -> impl Iterator<Item = PlayId> + '_ {
        self.slots.keys().copied()
    }

    pub fn plays(&self) -> impl Iterator<Item = &Play> {
        self.slots.values()
    }

    // unused for now, added for consistency
    #[allow(dead_code)]
    pub(super) fn plays_mut(&mut self) -> impl Iterator<Item = &mut Play> {
        self.slots.values_mut()
    }

    pub fn start(&self) -> &Options {
        &self.start
    }

    pub(super) fn start_mut(&mut self) -> &mut Options {
        &mut self.start
    }

    pub fn options(&self, id: PositionId) -> &Options {
        match id {
            PositionId::Start => self.start(),
            PositionId::Play(id) => &self.play(id).expect("play exists").options,
        }
    }

    pub(super) fn options_mut(&mut self, id: PositionId) -> &mut Options {
        match id {
            PositionId::Start => self.start_mut(),
            PositionId::Play(id) => &mut self.play_mut(id).expect("play exists").options,
        }
    }

    // Non-public, Game controls play coherence and legality.
    pub(super) fn insert(&mut self, f: impl FnOnce(PlayId) -> Play) -> PlayId {
        let slot = self.next;
        self.next += 1;

        let play = f(slot);
        debug_assert_eq!(slot, play.id);
        self.slots.insert(slot, play);
        slot
    }

    // Could plausibly be public, but would have to return bool to
    // avoid dangling pointers in the move's follow-on options.
    pub(super) fn remove(&mut self, id: PlayId) -> bool {
        let Some(play) = self.slots.remove(&id) else { return false };
        for option in play.options.iter() {
            self.remove(*option);
        }
        true
    }
}

impl PositionId {
    pub fn is_start(&self) -> bool {
        matches!(self, Self::Start)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for PositionId {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Start => Option::<PlayId>::None.serialize(serializer),
            Self::Play(id) => Some(*id).serialize(serializer),
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for PositionId {
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(match Option::<PlayId>::deserialize(deserializer)? {
            Some(id) => Self::Play(id),
            None => Self::Start,
        })
    }
}
