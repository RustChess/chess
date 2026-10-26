use std::collections::BTreeMap as Map;

use super::{self as game, *};

pub(in crate::game) trait Layout {
    fn reslot(self, game: &Game) -> Reslot;
}

impl Layout for game::Layout {
    fn reslot(self, game: &Game) -> Reslot {
        match self {
            Self::Pgn => pgn::reslot(game),
        }
    }
}

impl Reslot {
    pub(in crate::game) fn options(&self, position: &mut Position) {
        for slot in &mut position.options {
            *slot = self.play(*slot).expect("complete reslot");
        }
    }

    fn insert(&mut self, slot: MoveSlot) {
        let next = MoveSlot::from_index(self.len()).expect("game tree fits in move slots");
        self.0.insert(slot, next);
    }
}

mod pgn {
    use super::*;

    pub fn reslot(game: &Game) -> Reslot {
        let mut reslot = Reslot(Map::new());
        line(game.start(), &mut reslot);
        reslot
    }

    fn line(position: PositionRef<'_>, reslot: &mut Reslot) {
        let Ok(main) = position.main() else { return };

        reslot.insert(main.slot());
        for alternative in position.alternatives() {
            variation(alternative, reslot);
        }
        line(main.position(), reslot);
    }

    fn variation(play: MoveRef<'_>, reslot: &mut Reslot) {
        reslot.insert(play.slot());
        line(play.position(), reslot);
    }
}
