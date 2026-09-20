use core::ops::Neg;

use super::{self as game, *};

/// An error from applying a game patch.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("change tracking is disabled")]
    Disabled,
    #[error(transparent)]
    Insert(#[from] Insert),
}

/// An error from inserting a patched branch.
#[derive(Debug, thiserror::Error)]
pub enum Insert {
    #[error("move ID {0} is unavailable")]
    Unavailable(MoveId),
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
    Drag { position: PositionId, from: usize, to: usize },
    /// Exchanges two options.
    Swap { position: PositionId, a: usize, b: usize },
}

/// A reversible game change.
#[derive(Clone, PartialEq)]
pub enum Change {
    Game { before: GamePublic, after: GamePublic },
    Position { id: PositionId, before: PositionPublic, after: PositionPublic },
    Move { id: MoveId, before: MovePublic, after: MovePublic },
    Insert { position: PositionId, index: usize, branch: Branch },
    Remove { position: PositionId, index: usize, branch: Branch },
    Drag { position: PositionId, from: usize, to: usize },
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

impl Game {
    pub fn undo(&mut self) -> game::Result<()> {
        let Some(mut changes) = self.changes.take() else {
            return Err(Error::Disabled.into());
        };
        let Some(change) = changes.pop() else {
            self.changes = Some(changes);
            return Ok(());
        };
        let backward = -change;
        if let Err(error) = self.apply(&backward) {
            changes.push(-backward);
            self.changes = Some(changes);
            return Err(error);
        }
        self.changes = Some(changes);
        Ok(())
    }

    pub fn restore(&mut self, revision: usize) -> game::Result<()> {
        if !self.tracks_changes() {
            return Err(Error::Disabled.into());
        }
        while self.revisions().is_some_and(|revisions| revisions > revision) {
            self.undo()?;
        }
        Ok(())
    }

    pub fn patch(&mut self, patch: Patch) -> game::Result<()> {
        match patch {
            Patch::Game(public) => self.update_public(|current| *current = public),
            Patch::Position { id, public } => {
                self.position_mut(id)?.update_public(|current| *current = public);
            }
            Patch::Move { id, public } => {
                self.play_mut(id)?.update_public(|current| *current = public);
            }
            Patch::Insert { position, index, branch } => {
                self.insert_tracked(position, index, branch)?;
            }
            Patch::Remove(id) => {
                let position = self.play(id)?.previous().id();
                self.position_mut(position)?.remove(id)?;
            }
            Patch::Drag { position, from, to } => {
                self.position_mut(position)?.drag(from, to)?;
            }
            Patch::Swap { position, a, b } => {
                self.position_mut(position)?.swap(a, b)?;
            }
        }
        Ok(())
    }

    pub(in crate::game) fn track(&mut self, inverse: Patch) {
        let Some(mut changes) = self.changes.take() else { return };
        let change = -inverse.reversible(self).expect("recorded patch applies to the current game");
        changes.push(change);
        self.changes = Some(changes);
    }

    fn apply(&mut self, change: &Change) -> game::Result<()> {
        match change {
            Change::Game { after, .. } => self.public = after.clone(),
            Change::Position { id, after, .. } => {
                self.position_mut(*id)?.position_mut().public = after.clone();
            }
            Change::Move { id, after, .. } => {
                self.play_mut(*id)?.play_mut().public = after.clone();
            }
            Change::Drag { position, from, to } => {
                self.position_mut(*position)?.drag_untracked(*from, *to)?;
            }
            Change::Swap { position, a, b } => {
                self.position_mut(*position)?.swap_untracked(*a, *b)?;
            }
            Change::Remove { branch, .. } => {
                let position = self.play(branch.id)?.previous().id();
                self.position_mut(position)?.remove_untracked(branch.id)?;
            }
            Change::Insert { position, index, branch } => {
                self.insert_untracked(*position, *index, branch)?;
            }
        }
        Ok(())
    }

    fn insert_tracked(
        &mut self,
        position: PositionId,
        index: usize,
        branch: Branch,
    ) -> game::Result<()> {
        self.insert_untracked(position, index, &branch)?;
        if let Some(changes) = self.changes.as_mut() {
            changes.push(Change::Insert { position, index, branch });
        }
        Ok(())
    }

    fn insert_untracked(
        &mut self,
        position: PositionId,
        index: usize,
        branch: &Branch,
    ) -> game::Result<()> {
        let parent = self.position(position)?;
        if index > parent.options().len() {
            let missing = traversal::Missing { position, choice: index.into() };
            return Err(traversal::Error::from(missing).into());
        }
        if let Ok(index) = parent.index(branch.play) {
            let duplicate = options::Insert::Duplicate(index);
            return Err(options::Error::from(duplicate).into());
        }

        let mut trial = parent.into_game();
        insert_branch(&mut trial, Some(self), START, 0, branch)?;
        insert_branch(self, None, position, index, branch)
    }
}

impl Patch {
    fn reversible(self, game: &Game) -> traversal::Result<Change> {
        Ok(match self {
            Self::Game(after) => Change::Game { before: game.public.clone(), after },
            Self::Position { id, public: after } => {
                Change::Position { id, before: game.position(id)?.public.clone(), after }
            }
            Self::Move { id, public: after } => {
                Change::Move { id, before: game.play(id)?.public.clone(), after }
            }
            Self::Insert { position, index, branch } => Change::Insert { position, index, branch },
            Self::Remove(id) => {
                let position = game.play(id)?.previous();
                let index = position.index(id)?;
                Change::Remove { position: position.id(), index, branch: game.tree.branch(id) }
            }
            Self::Drag { position, from, to } => Change::Drag { position, from, to },
            Self::Swap { position, a, b } => Change::Swap { position, a, b },
        })
    }
}

impl Change {
    pub fn forward(&self) -> Patch {
        match self {
            Self::Game { after, .. } => Patch::Game(after.clone()),
            Self::Position { id, after, .. } => Patch::Position { id: *id, public: after.clone() },
            Self::Move { id, after, .. } => Patch::Move { id: *id, public: after.clone() },
            Self::Insert { position, index, branch } => {
                Patch::Insert { position: *position, index: *index, branch: branch.clone() }
            }
            Self::Remove { branch, .. } => Patch::Remove(branch.id),
            Self::Drag { position, from, to } => {
                Patch::Drag { position: *position, from: *from, to: *to }
            }
            Self::Swap { position, a, b } => Patch::Swap { position: *position, a: *a, b: *b },
        }
    }

    pub fn backward(&self) -> Patch {
        match self {
            Self::Game { before, .. } => Patch::Game(before.clone()),
            Self::Position { id, before, .. } => {
                Patch::Position { id: *id, public: before.clone() }
            }
            Self::Move { id, before, .. } => Patch::Move { id: *id, public: before.clone() },
            Self::Insert { branch, .. } => Patch::Remove(branch.id),
            Self::Remove { position, index, branch } => {
                Patch::Insert { position: *position, index: *index, branch: branch.clone() }
            }
            Self::Drag { position, from, to } => {
                Patch::Drag { position: *position, from: *to, to: *from }
            }
            Self::Swap { position, a, b } => Patch::Swap { position: *position, a: *a, b: *b },
        }
    }
}

impl Neg for Change {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Self::Game { before, after } => Self::Game { before: after, after: before },
            Self::Position { id, before, after } => {
                Self::Position { id, before: after, after: before }
            }
            Self::Move { id, before, after } => Self::Move { id, before: after, after: before },
            Self::Insert { position, index, branch } => Self::Remove { position, index, branch },
            Self::Remove { position, index, branch } => Self::Insert { position, index, branch },
            Self::Drag { position, from, to } => Self::Drag { position, from: to, to: from },
            Self::Swap { position, a, b } => Self::Swap { position, a, b },
        }
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
        formats::Pgn,
        square::Square::*,
    };

    use super::*;

    #[test]
    fn game_change_tracking() {
        let mut game = Game::new(Position::start());
        assert!(!game.tracks_changes());
        assert_eq!(game.revisions(), None);
        assert!(matches!(game.undo(), Err(game::Error::Change(Error::Disabled))));
        assert!(matches!(game.restore(0), Err(game::Error::Change(Error::Disabled))));

        game.track_changes(true);
        assert!(game.tracks_changes());
        assert_eq!(game.revisions(), Some(0));
        game.track_changes(false);
        assert!(!game.tracks_changes());
        assert!(game.changes().is_none());
    }

    #[test]
    fn game_public_changes() {
        let mut game = Game::new(Position::start());
        game.set_orientation(Player::Black);
        assert!(game.changes().is_none());

        game.track_changes(true);
        game.set_orientation(Player::Black);
        game.update_roster(|_| ());
        game.update_tags(|_| ());
        assert!(game.changes().unwrap().is_empty());

        game.set_orientation(Player::White);
        game.set_outcome(Outcome::Draw);
        game.update_roster(|roster| roster.event = Some("Event".parse().unwrap()));
        game.update_tags(|tags| {
            tags.push(Tag { key: "Annotator".parse().unwrap(), value: "Player".to_string() });
        });
        assert_eq!(game.changes().unwrap().len(), 4);

        let Change::Game { before, after } = &game.changes().unwrap()[0] else { panic!() };
        assert_eq!(before.orientation, Player::Black);
        assert_eq!(after.orientation, Player::White);

        let Change::Game { before, after } = &game.changes().unwrap()[3] else { panic!() };
        assert!(before.tags.is_empty());
        assert_eq!(after.tags.len(), 1);
    }

    #[test]
    fn position_public_changes() {
        let mut game = Game::new(Position::start());
        let comment: Text = "Comment".parse().unwrap();
        game.start_mut().set_comment(Some(comment.clone()));
        assert!(game.changes().is_none());

        game.track_changes(true);
        game.start_mut().set_comment(Some(comment));
        assert!(game.changes().unwrap().is_empty());

        game.start_mut().set_comment(None);
        game.start_mut()
            .set_valuation(Some(Valuation { depth: Some(1), score: Score::Centipawns(10) }));
        game.start_mut().set_expanded(true);
        assert_eq!(game.changes().unwrap().len(), 3);

        let Change::Position { id, before, after } = &game.changes().unwrap()[0] else { panic!() };
        assert_eq!(*id, START);
        assert!(before.comment.is_some());
        assert!(after.comment.is_none());
    }

    #[test]
    fn move_public_changes() {
        let mut game = Game::new(Position::start());
        let id = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let intro: Text = "Intro".parse().unwrap();
        game.play_mut(id).unwrap().set_intro(Some(intro.clone()));
        assert!(game.changes().is_none());

        game.track_changes(true);
        game.play_mut(id).unwrap().set_intro(Some(intro));
        assert!(game.changes().unwrap().is_empty());

        game.play_mut(id).unwrap().set_intro(None);
        game.play_mut(id).unwrap().set_outro(Some("Outro".parse().unwrap()));
        game.play_mut(id).unwrap().update_commands(|commands| commands.push(Command::expanded()));
        game.play_mut(id).unwrap().update_nags(|nags| nags.push(Nag::Numeric(1)));
        assert_eq!(game.changes().unwrap().len(), 4);

        let Change::Move { id: patched, before, after } = &game.changes().unwrap()[0] else {
            panic!()
        };
        assert_eq!(*patched, id);
        assert!(before.affixes.intro.is_some());
        assert!(after.affixes.intro.is_none());
    }

    #[test]
    fn option_order_changes() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let d4 = game.start_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap().id();
        let c4 = game.start_mut().push(crate::Move::normal(Pawn, C2, C4)).unwrap().id();

        game.track_changes(true);
        assert!(!game.start_mut().promote(e4).unwrap());
        assert!(game.changes().unwrap().is_empty());

        assert!(game.start_mut().promote(c4).unwrap());
        assert!(game.start_mut().swap(c4, d4).unwrap());
        assert_eq!(game.changes().unwrap().len(), 2);

        let Change::Drag { position, from, to } = game.changes().unwrap()[0] else { panic!() };
        assert_eq!(position, START);
        assert_eq!((from, to), (2, 0));

        let Change::Swap { position, a, b } = game.changes().unwrap()[1] else { panic!() };
        assert_eq!(position, START);
        assert_eq!((a, b), (0, 2));
    }

    #[test]
    fn apply_public_patch() {
        let mut game = Game::new(Position::start());
        game.track_changes(true);
        game.set_outcome(Outcome::Draw);

        let draw = game.public.clone();
        let change = game.changes().unwrap()[0].clone();
        game.apply(&-change).unwrap();
        assert_eq!(game.outcome(), Outcome::Unknown);
        assert_eq!(game.changes().unwrap().len(), 1);

        let change = Patch::Game(draw).reversible(&game).unwrap();
        game.apply(&change).unwrap();
        assert_eq!(game.outcome(), Outcome::Draw);
        assert_eq!(game.changes().unwrap().len(), 1);
    }

    #[test]
    fn patch_records_actual_state() {
        let mut game = Game::new(Position::start());
        let mut public = game.public.clone();
        public.outcome = Outcome::Draw;
        game.patch(Patch::Game(public.clone())).unwrap();
        assert!(game.changes().is_none());

        game.track_changes(true);
        public.outcome = Outcome::White;
        game.patch(Patch::Game(public)).unwrap();

        let Change::Game { before, after } = &game.changes().unwrap()[0] else { panic!() };
        assert_eq!(before.outcome, Outcome::Draw);
        assert_eq!(after.outcome, Outcome::White);
    }

    #[test]
    fn replay_forward_history() {
        let mut game = Game::new(Position::start());
        let base = game.clone();
        game.track_changes(true);
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        game.play_mut(e4).unwrap().set_comment(Some("King's pawn".parse().unwrap()));
        game.play_mut(e4).unwrap().position_mut().push(crate::Move::normal(Pawn, E7, E5)).unwrap();
        game.set_outcome(Outcome::Draw);

        let mut replay = base;
        for change in game.changes().unwrap() {
            replay.apply(change).unwrap();
        }

        assert_eq!(Pgn::from(replay), Pgn::from(game));
    }

    #[test]
    fn apply_order_patch() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let d4 = game.start_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap().id();
        let c4 = game.start_mut().push(crate::Move::normal(Pawn, C2, C4)).unwrap().id();
        game.track_changes(true);
        game.start_mut().promote(c4).unwrap();

        let change = game.changes().unwrap()[0].clone();
        game.apply(&-change).unwrap();
        assert_eq!(game.start().options(), [e4, d4, c4]);
        assert_eq!(game.changes().unwrap().len(), 1);
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
        game.track_changes(true);

        game.patch(Patch::Remove(e4)).unwrap();
        assert!(game.play(e4).is_err());
        assert!(game.play(e5).is_err());

        let Change::Remove { position, index, branch } = &game.changes().unwrap()[0] else {
            panic!()
        };
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

        game.track_changes(true);
        game.start_mut().remove(e4).unwrap();
        assert!(game.play(e4).is_err());
        assert!(game.play(e5).is_err());
        assert_eq!(game.changes().unwrap().len(), 1);

        let Change::Remove { position, index, branch } = &game.changes().unwrap()[0] else {
            panic!()
        };
        assert_eq!(*position, START);
        assert_eq!(*index, 0);
        assert_eq!(branch.id, e4);
        assert_eq!(branch.options[0].id, e5);

        assert!(game.start_mut().remove(e4).is_err());
        assert_eq!(game.changes().unwrap().len(), 1);
    }

    #[test]
    fn insert_patch() {
        let mut game = Game::new(Position::start());
        let play = crate::Move::normal(Pawn, E2, E4);
        game.track_changes(true);

        let e4 = game.start_mut().push(play).unwrap().id();
        assert_eq!(game.changes().unwrap().len(), 1);
        let Change::Insert { branch, .. } = &game.changes().unwrap()[0] else { panic!() };
        assert_eq!(branch.id, e4);
        assert_eq!(game.start().options(), [e4]);

        assert!(game.start_mut().push(play).is_err());
        assert_eq!(game.changes().unwrap().len(), 1);
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
            game::Error::Change(Error::Insert(Insert::Unavailable(id))) if id == e4
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
        game.play_mut(e4).unwrap().position_mut().push(crate::Move::normal(Pawn, E7, E5)).unwrap();
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
    fn insert_branch_records_one_change() {
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
        game.track_changes(true);
        game.patch(Patch::Insert { position: START, index: 0, branch }).unwrap();

        assert_eq!(game.changes().unwrap().len(), 1);
        assert!(matches!(
            game.changes().unwrap()[0],
            Change::Insert { ref branch, .. } if branch.id == e4
        ));
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
                    && missing.choice == traversal::Choice::Index(1)
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
            game::Error::Change(Error::Insert(Insert::Unavailable(id))) if id == e4
        ));
        assert!(game == before);
    }

    #[test]
    fn undo_public_mutation() {
        let mut game = Game::new(Position::start());
        game.track_changes(true);
        game.set_outcome(Outcome::Draw);

        game.undo().unwrap();
        assert_eq!(game.outcome(), Outcome::Unknown);
        assert!(game.changes().unwrap().is_empty());
        game.undo().unwrap();
    }

    #[test]
    fn compact_clears_history() {
        let mut game = Game::new(Position::start());
        game.track_changes(true);
        game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap();

        game.compact(Layout::Pgn);

        assert!(game.changes().unwrap().is_empty());
        game.set_outcome(Outcome::Draw);
        assert_eq!(game.changes().unwrap().len(), 1);
    }

    #[test]
    fn undo_branch_removal() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        game.play_mut(e4).unwrap().position_mut().push(crate::Move::normal(Pawn, E7, E5)).unwrap();
        game.track_changes(true);
        let before = game.clone();

        game.start_mut().remove(e4).unwrap();
        game.undo().unwrap();

        assert!(game == before);
    }

    #[test]
    fn restore_revision() {
        let mut game = Game::new(Position::start());
        game.track_changes(true);
        game.set_orientation(Player::Black);
        game.set_outcome(Outcome::Draw);

        game.restore(1).unwrap();
        assert_eq!(game.orientation(), Player::Black);
        assert_eq!(game.outcome(), Outcome::Unknown);
        assert_eq!(game.changes().unwrap().len(), 1);

        game.restore(2).unwrap();
        assert_eq!(game.changes().unwrap().len(), 1);

        game.restore(0).unwrap();
        assert_eq!(game.orientation(), Player::White);
        assert!(game.changes().unwrap().is_empty());
    }

    #[test]
    fn undo_position_and_move_public() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        game.track_changes(true);

        game.start_mut().set_comment(Some("Position".parse().unwrap()));
        game.play_mut(e4).unwrap().set_intro(Some("Move".parse().unwrap()));
        game.restore(0).unwrap();

        assert!(game.start().comment().is_none());
        assert!(game.play(e4).unwrap().public.affixes.intro.is_none());
    }

    #[test]
    fn undo_pushed_move() {
        let mut game = Game::new(Position::start());
        game.track_changes(true);
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();

        game.undo().unwrap();
        assert!(game.start().options().is_empty());
        assert!(game.play(e4).is_err());
    }

    #[test]
    fn undo_option_order() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let d4 = game.start_mut().push(crate::Move::normal(Pawn, D2, D4)).unwrap().id();
        let c4 = game.start_mut().push(crate::Move::normal(Pawn, C2, C4)).unwrap().id();
        game.track_changes(true);
        let before = game.clone();

        game.start_mut().promote(c4).unwrap();
        game.start_mut().swap(e4, d4).unwrap();
        game.restore(0).unwrap();

        assert!(game == before);
    }

    #[test]
    fn failed_undo_restores_change() {
        let mut game = Game::new(Position::start());
        let e4 = game.start_mut().push(crate::Move::normal(Pawn, E2, E4)).unwrap().id();
        let branch = game.tree.branch(e4);
        game.start_mut().remove(e4).unwrap();
        game.track_changes(true);
        game.changes.as_mut().unwrap().push(Change::Insert { position: START, index: 0, branch });

        let error = game.undo().unwrap_err();

        assert!(matches!(
            error,
            game::Error::Traversal(traversal::Error::Move(id)) if id == e4
        ));
        assert!(matches!(
            game.changes().unwrap(),
            [Change::Insert { branch, .. }] if branch.id == e4
        ));
    }
}
