use super::{self as game, *};

/// An error from applying a game patch.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Insert(#[from] Insert),
}

/// An error from inserting a patched branch.
#[derive(Debug, thiserror::Error)]
pub enum Insert {
    #[error("move ID {0:?} is unavailable")]
    Unavailable(MoveId),
}

/// Undo state for a game.
#[derive(Clone, Default, PartialEq)]
pub struct Undo {
    pub on: bool,
    pub stack: Vec<Patch>,
}

/// A game mutation.
#[derive(Clone, PartialEq)]
pub enum Patch {
    /// Replaces public game data.
    Game(GamePublic),
    /// Replaces public position data.
    Position { id: PositionId, public: PositionPublic },
    /// Replaces public move data.
    Move { id: MoveId, public: MovePublic },
    /// Inserts a branch at an option index.
    Insert { position: PositionId, index: usize, branch: Branch },
    /// Removes the branch rooted at a move.
    Remove(MoveId),
    /// Moves an option between indices.
    Reorder { position: PositionId, from: usize, to: usize },
    /// Exchanges two options.
    Swap { position: PositionId, a: usize, b: usize },
}

/// A move and the complete game subtree following it.
#[derive(Clone, PartialEq)]
pub struct Branch {
    /// The move's assigned tree slot.
    pub id: MoveId,
    /// The chess move.
    pub play: chess::Move,
    /// Public data associated with the move and resulting position.
    pub public: BranchPublic,
    /// Ordered continuations from the resulting position.
    pub options: Vec<Branch>,
}

/// Public data associated with a branch root.
#[derive(Clone, PartialEq)]
pub struct BranchPublic {
    /// Public data associated with the move.
    pub play: MovePublic,
    /// Public data associated with the resulting position.
    pub position: PositionPublic,
}

/// Canonical tree-slot assignment order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Order {
    /// The move encounter order used while constructing a game from PGN.
    Pgn,
}

impl Game {
    pub fn patch(&mut self, patch: Patch) -> game::Result<()> {
        let inverse = if self.undo.on { Some(self.invert(&patch)?) } else { None };
        self.apply(patch)?;
        if let Some(inverse) = inverse {
            self.undo.stack.push(inverse);
        }
        Ok(())
    }

    fn invert(&self, patch: &Patch) -> traversal::Result<Patch> {
        Ok(match patch {
            // infallible
            Patch::Game(_) => Patch::Game(self.public.clone()),
            // Position(id)
            Patch::Position { id, .. } => {
                Patch::Position { id: *id, public: self.position(*id)?.public.clone() }
            }
            // Move(id)
            Patch::Move { id, .. } => {
                Patch::Move { id: *id, public: self.play(*id)?.public.clone() }
            }
            Patch::Insert { branch, .. } => Patch::Remove(branch.id),
            // Move(id)
            Patch::Remove(id) => {
                let position = self.play(*id)?.previous();
                // Infallible by internal consistency
                let index = position.index(*id)?;
                // self.tree.branch recursively collects the branch under `id`
                Patch::Insert { position: position.id(), index, branch: self.tree.branch(*id) }
            }
            // infallible
            Patch::Reorder { position, from, to } => {
                Patch::Reorder { position: *position, from: *to, to: *from }
            }
            // infallible
            Patch::Swap { position, a, b } => Patch::Swap { position: *position, a: *a, b: *b },
        })
    }

    fn apply(&mut self, patch: Patch) -> game::Result<()> {
        match patch {
            Patch::Game(public) => self.public = public,
            Patch::Position { id, public } => {
                self.position_mut(id)?.position_mut().public = public;
            }
            Patch::Move { id, public } => {
                self.play_mut(id)?.play_mut().public = public;
            }
            Patch::Reorder { position, from, to } => {
                let mut position = self.position_mut(position)?;
                position.reorder_untracked(from, to)?;
            }
            Patch::Swap { position, a, b } => {
                let mut position = self.position_mut(position)?;
                position.swap_untracked(a, b)?;
            }
            Patch::Remove(id) => {
                let position = self.play(id)?.previous().id();
                self.position_mut(position)?.remove_untracked(id)?;
            }
            Patch::Insert { position, index, branch } => self.insert(position, index, branch)?,
        }
        Ok(())
    }

    fn insert(&mut self, position: PositionId, index: usize, branch: Branch) -> game::Result<()> {
        let parent = self.position(position)?;
        if index > parent.options().len() {
            let missing = traversal::MissingOption { position, location: index.into() };
            return Err(traversal::Error::from(missing).into());
        }
        if let Ok(index) = parent.index(branch.play) {
            let duplicate = options::Insert::Duplicate(index);
            return Err(options::Error::from(duplicate).into());
        }

        let mut trial = parent.into_game();
        insert_branch(&mut trial, Some(self), START, 0, &branch)?;
        insert_branch(self, None, position, index, &branch)
    }
}

fn insert_branch(
    game: &mut Game,
    source: Option<&Game>,
    position: PositionId,
    index: usize,
    branch: &Branch,
) -> game::Result<()> {
    let unavailable_in_source = source.is_some_and(|source| source.play(branch.id).is_ok());
    let unavailable_in_game = game.play(branch.id).is_ok();
    if unavailable_in_source || unavailable_in_game {
        return Err(Error::Insert(Insert::Unavailable(branch.id)).into());
    }

    game.position_mut(position)?.insert_at_untracked(Some(branch.id), index, branch.play)?;
    let mut play = game.play_mut(branch.id)?;
    play.play_mut().public = branch.public.play.clone();
    play.position_mut().position_mut().public = branch.public.position.clone();

    for (index, option) in branch.options.iter().enumerate() {
        insert_branch(game, source, branch.id.into(), index, option)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use arbitrary::{Arbitrary as _, Unstructured};

    use crate::{
        Game, Player, Position,
        board::Role::{Knight, Pawn},
        square::Square::*,
    };

    use super::*;

    #[test]
    fn undo() {
        let undo = Undo::default();
        assert!(!undo.on);
        assert!(undo.stack.is_empty());
    }

    #[test]
    fn game_undo_switch() {
        let mut game = Game::new(Position::start());
        assert!(!game.has_undo());

        game.set_undo(true);
        assert!(game.has_undo());
        game.set_undo(false);
        assert!(!game.has_undo());
        assert!(game.patches().is_empty());
    }

    #[test]
    fn game_public_patches() {
        let mut game = Game::new(Position::start());
        game.set_orientation(Player::Black);
        assert!(game.patches().is_empty());

        game.set_undo(true);
        game.set_orientation(Player::Black);
        game.update_roster(|_| ());
        game.update_tags(|_| ());
        assert!(game.patches().is_empty());

        game.set_orientation(Player::White);
        game.set_outcome(Outcome::Draw);
        game.update_roster(|roster| roster.event = Some("Event".parse().unwrap()));
        game.update_tags(|tags| {
            tags.push(Tag { key: "Annotator".parse().unwrap(), value: "Player".to_string() });
        });
        assert_eq!(game.patches().len(), 4);

        let Patch::Game(public) = &game.patches()[0] else { panic!() };
        assert_eq!(public.orientation, Player::Black);

        let Patch::Game(public) = &game.patches()[3] else { panic!() };
        assert!(public.tags.is_empty());
    }

    #[test]
    fn position_public_patches() {
        let mut game = Game::new(Position::start());
        let comment: Text = "Comment".parse().unwrap();
        game.start_mut().set_comment(Some(comment.clone()));
        assert!(game.patches().is_empty());

        game.set_undo(true);
        game.start_mut().set_comment(Some(comment));
        assert!(game.patches().is_empty());

        game.start_mut().set_comment(None);
        game.start_mut()
            .set_valuation(Some(Valuation { depth: Some(1), score: Score::Centipawns(10) }));
        game.start_mut().set_expanded(true);
        assert_eq!(game.patches().len(), 3);

        let Patch::Position { id, public } = &game.patches()[0] else { panic!() };
        assert_eq!(*id, START);
        assert!(public.comment.is_some());
    }

    #[test]
    fn move_public_patches() {
        let mut game = Game::new(Position::start());
        let id = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let intro: Text = "Intro".parse().unwrap();
        game.play_mut(id).unwrap().set_intro(Some(intro.clone()));
        assert!(game.patches().is_empty());

        game.set_undo(true);
        game.play_mut(id).unwrap().set_intro(Some(intro));
        assert!(game.patches().is_empty());

        game.play_mut(id).unwrap().set_intro(None);
        game.play_mut(id).unwrap().set_outro(Some("Outro".parse().unwrap()));
        game.play_mut(id).unwrap().update_commands(|commands| commands.push(Command::expanded()));
        game.play_mut(id).unwrap().update_nags(|nags| nags.push(Nag::Numeric(1)));
        assert_eq!(game.patches().len(), 4);

        let Patch::Move { id: patched, public } = &game.patches()[0] else { panic!() };
        assert_eq!(*patched, id);
        assert!(public.affixes.intro.is_some());
    }

    #[test]
    fn option_order_patches() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let d4 = game.start_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap().id();
        let c4 = game.start_mut().push(crate::Move::normal(Pawn, C2, C4)).unwrap().id();

        game.set_undo(true);
        assert!(!game.start_mut().promote(e4).unwrap());
        assert!(game.patches().is_empty());

        assert!(game.start_mut().promote(c4).unwrap());
        assert!(game.start_mut().swap(c4, d4).unwrap());
        assert_eq!(game.patches().len(), 2);

        let Patch::Reorder { position, from, to } = game.patches()[0] else { panic!() };
        assert_eq!(position, START);
        assert_eq!((from, to), (0, 2));

        let Patch::Swap { position, a, b } = game.patches()[1] else { panic!() };
        assert_eq!(position, START);
        assert_eq!((a, b), (0, 2));
    }

    #[test]
    fn apply_public_patch() {
        let mut game = Game::new(Position::start());
        game.set_undo(true);
        game.set_outcome(Outcome::Draw);

        let draw = game.public.clone();
        let patch = game.patches()[0].clone();
        game.apply(patch).unwrap();
        assert_eq!(game.outcome(), Outcome::Unknown);
        assert_eq!(game.patches().len(), 1);

        game.apply(Patch::Game(draw)).unwrap();
        assert_eq!(game.outcome(), Outcome::Draw);
        assert_eq!(game.patches().len(), 1);
    }

    #[test]
    fn patch_records_actual_state() {
        let mut game = Game::new(Position::start());
        let mut public = game.public.clone();
        public.outcome = Outcome::Draw;
        game.patch(Patch::Game(public.clone())).unwrap();
        assert!(game.patches().is_empty());

        game.set_undo(true);
        public.outcome = Outcome::White;
        game.patch(Patch::Game(public)).unwrap();

        let Patch::Game(public) = &game.patches()[0] else { panic!() };
        assert_eq!(public.outcome, Outcome::Draw);
    }

    #[test]
    fn apply_order_patch() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let d4 = game.start_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap().id();
        let c4 = game.start_mut().push(crate::Move::normal(Pawn, C2, C4)).unwrap().id();
        game.set_undo(true);
        game.start_mut().promote(c4).unwrap();

        let patch = game.patches()[0].clone();
        game.apply(patch).unwrap();
        assert_eq!(game.start().options(), [e4, d4, c4]);
        assert_eq!(game.patches().len(), 1);
    }

    #[test]
    fn apply_remove_patch() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap()
            .id();
        game.set_undo(true);

        game.patch(Patch::Remove(e4)).unwrap();
        assert!(game.play(e4).is_err());
        assert!(game.play(e5).is_err());

        let Patch::Insert { position, index, branch } = &game.patches()[0] else { panic!() };
        assert_eq!(*position, START);
        assert_eq!(*index, 0);
        assert_eq!(branch.id, e4);
        assert_eq!(branch.options[0].id, e5);
    }

    #[test]
    fn branch_capture() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap()
            .id();
        let c5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, C7, C5))
            .unwrap()
            .id();
        let nf3 = game
            .play_mut(e5)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Knight, G1, F3))
            .unwrap()
            .id();
        game.play_mut(e4).unwrap().set_intro(Some("Intro".parse().unwrap()));
        game.play_mut(e4).unwrap().set_comment(Some("Comment".parse().unwrap()));

        let branch = game.tree.branch(e4);
        assert_eq!(branch.id, e4);
        assert_eq!(branch.play, crate::Move::normal(Pawn, E2, E4));
        assert!(branch.public.play.affixes.intro.is_some());
        assert!(branch.public.position.comment.is_some());
        assert_eq!(branch.options.iter().map(|branch| branch.id).collect::<Vec<_>>(), [e5, c5]);
        assert_eq!(branch.options[0].options[0].id, nf3);
    }

    #[test]
    fn remove_patch() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap()
            .id();

        game.set_undo(true);
        game.start_mut().remove(e4).unwrap();
        assert!(game.play(e4).is_err());
        assert!(game.play(e5).is_err());
        assert_eq!(game.patches().len(), 1);

        let Patch::Insert { position, index, branch } = &game.patches()[0] else { panic!() };
        assert_eq!(*position, START);
        assert_eq!(*index, 0);
        assert_eq!(branch.id, e4);
        assert_eq!(branch.options[0].id, e5);

        assert!(game.start_mut().remove(e4).is_err());
        assert_eq!(game.patches().len(), 1);
    }

    #[test]
    fn insert_patch() {
        let mut game = Game::new(Position::start());
        let play = crate::Move::normal(Pawn, E2, E4);
        game.set_undo(true);

        let e4 = game.start_mut().push(play).unwrap().id();
        assert_eq!(game.patches().len(), 1);
        let Patch::Remove(id) = game.patches()[0] else { panic!() };
        assert_eq!(id, e4);
        assert_eq!(game.start().options(), [e4]);

        assert!(game.start_mut().push(play).is_err());
        assert_eq!(game.patches().len(), 1);
    }

    #[test]
    fn insert_branch_round_trip() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap()
            .id();
        game.play_mut(e4).unwrap().position_mut().push(crate::Move::normal(Pawn, C7, C5)).unwrap();
        game.play_mut(e5)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Knight, G1, F3))
            .unwrap();
        game.play_mut(e4).unwrap().set_intro(Some("Intro".parse().unwrap()));
        game.play_mut(e5).unwrap().set_comment(Some("Comment".parse().unwrap()));

        let before = game.clone();
        let branch = game.tree.branch(e4);
        game.start_mut().remove(e4).unwrap();
        game.patch(Patch::Insert { position: START, index: 0, branch }).unwrap();

        assert!(game == before);
    }

    #[test]
    fn failed_insert_is_atomic() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        game.play_mut(e4).unwrap().position_mut().push(crate::Move::normal(Pawn, E7, E5)).unwrap();
        let mut branch = game.tree.branch(e4);
        game.start_mut().remove(e4).unwrap();
        let before = game.clone();
        branch.options[0].play = crate::Move::normal(Pawn, E2, E4);

        let error = game.patch(Patch::Insert { position: START, index: 0, branch }).unwrap_err();

        assert!(matches!(
            error,
            game::Error::Options(options::Error::Insert(options::Insert::Illegal))
        ));
        assert!(game == before);
    }

    #[test]
    fn unavailable_insert_is_atomic() {
        let mut source = Game::new(Position::start());
        let e4 = source.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let branch = source.tree.branch(e4);

        let mut game = Game::new(Position::start());
        game.start_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap();
        let before = game.clone();

        let error = game.patch(Patch::Insert { position: START, index: 1, branch }).unwrap_err();

        assert!(matches!(
            error,
            game::Error::Patch(Error::Insert(Insert::Unavailable(id))) if id == e4
        ));
        assert!(game == before);
    }

    #[test]
    fn into_game() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let d4 = game.start_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap().id();
        let e5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap()
            .id();

        let mut position = game.play(e4).unwrap().position().into_game();
        assert!(position.start().options().is_empty());
        let position_e5 =
            position.start_mut().push(crate::Move::normal(Pawn, E7, E5)).unwrap().id();
        assert_eq!(position_e5, e5.next().unwrap());

        let mut play = game.play(e4).unwrap().into_game();
        assert_eq!(play.start().options(), [e4]);
        assert!(play.play(e4).unwrap().position().options().is_empty());
        assert!(play.play(d4).is_err());
        assert!(play.play(e5).is_err());
        let play_e5 = play
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap()
            .id();
        assert_eq!(play_e5, e5.next().unwrap());
    }

    #[test]
    fn arbitrary_insert_round_trip() {
        let mut tested = 0;
        for seed in 0_u8..100 {
            let bytes = (0..1024)
                .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
                .collect::<Vec<_>>();
            let mut input = Unstructured::new(&bytes);
            let Ok(mut game) = Game::arbitrary(&mut input) else { continue };
            let Some(id) = game.start().iter().next().map(|play| play.id()) else { continue };

            let before = game.clone();
            let branch = game.tree.branch(id);
            game.start_mut().remove(id).unwrap();
            game.patch(Patch::Insert { position: START, index: 0, branch }).unwrap();
            assert!(game == before);

            tested += 1;
            if tested == 10 {
                break;
            }
        }
        assert_eq!(tested, 10);
    }

    #[test]
    fn insert_branch_at_deep_nonzero_index() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        game.play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap();
        let c5 = game
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, C7, C5))
            .unwrap()
            .id();
        let before = game.clone();
        let branch = game.tree.branch(c5);

        game.play_mut(e4).unwrap().position_mut().remove(c5).unwrap();
        game.patch(Patch::Insert { position: e4.into(), index: 1, branch }).unwrap();

        assert!(game == before);
    }

    #[test]
    fn insert_branch_records_one_inverse() {
        let mut source = Game::new(Position::start());
        let e4 = source.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        source
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap();
        let branch = source.tree.branch(e4);

        let mut game = Game::new(Position::start());
        game.set_undo(true);
        game.patch(Patch::Insert { position: START, index: 0, branch }).unwrap();

        assert_eq!(game.patches().len(), 1);
        assert!(matches!(game.patches()[0], Patch::Remove(id) if id == e4));
    }

    #[test]
    fn out_of_range_insert_is_atomic() {
        let mut source = Game::new(Position::start());
        let e4 = source.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let branch = source.tree.branch(e4);

        let mut game = Game::new(Position::start());
        let before = game.clone();
        let error = game.patch(Patch::Insert { position: START, index: 1, branch }).unwrap_err();

        assert!(matches!(
            error,
            game::Error::Traversal(traversal::Error::Option(ref missing))
                if missing.position == START
                    && missing.location == traversal::Location::Index(1)
        ));
        assert!(game == before);
    }

    #[test]
    fn duplicate_branch_id_is_atomic() {
        let mut source = Game::new(Position::start());
        let e4 = source.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        source
            .play_mut(e4)
            .unwrap()
            .position_mut()
            .push(crate::Move::normal(Pawn, E7, E5))
            .unwrap();
        let mut branch = source.tree.branch(e4);
        branch.options[0].id = e4;

        let mut game = Game::new(Position::start());
        let before = game.clone();
        let error = game.patch(Patch::Insert { position: START, index: 0, branch }).unwrap_err();

        assert!(matches!(
            error,
            game::Error::Patch(Error::Insert(Insert::Unavailable(id))) if id == e4
        ));
        assert!(game == before);
    }
}
