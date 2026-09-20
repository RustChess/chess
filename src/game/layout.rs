use std::collections::BTreeMap as Map;

use super::{self as game, *};

pub(in crate::game) trait Layout {
    fn compact(self, game: &Game) -> Compact;
}

impl Layout for game::Layout {
    fn compact(self, game: &Game) -> Compact {
        match self {
            Self::Pgn => pgn::compact(game),
        }
    }
}

impl Compact {
    pub(in crate::game) fn options(&self, position: &mut Position) {
        for id in &mut position.options {
            *id = self.play(*id).expect("complete compaction");
        }
    }

    fn insert(&mut self, id: MoveId) {
        let next = MoveId::from_index(self.len()).expect("game tree fits in move IDs");
        self.0.insert(id, next);
    }
}

mod pgn {
    use super::*;

    pub fn compact(game: &Game) -> Compact {
        let mut compact = Compact(Map::new());
        line(game.start(), &mut compact);
        compact
    }

    fn line(position: PositionRef<'_>, compact: &mut Compact) {
        let Ok(main) = position.main() else { return };

        compact.insert(main.id());
        for alternative in position.alternatives() {
            variation(alternative, compact);
        }
        line(main.position(), compact);
    }

    fn variation(play: MoveRef<'_>, compact: &mut Compact) {
        compact.insert(play.id());
        line(play.position(), compact);
    }
}
