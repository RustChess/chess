use crate::Position;
use crate::game::{self, Key, Node, Tag};

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("SAN error: {0}")]
    San(#[from] san::Error),
    #[error("game error: {0}")]
    Game(#[from] game::Error),
    #[error("invalid FEN: {0}")]
    Fen(String),
    #[error("empty PGN variation of {} {san}", MoveNumber(*ply))]
    EmptyVariation { ply: usize, san: san::San },
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl Game {
    pub fn start(&self) -> Option<Position> {
        game_position(&self.tags).ok()
    }

    pub fn first_ply(&self) -> usize {
        self.start().map(|position| position.first_ply()).unwrap_or(0)
    }
}

impl From<crate::Game> for Game {
    fn from(game: crate::Game) -> Self {
        let start = game.start();
        let moves = pgn_moves(&game, game.start_options());
        let mut tags = game.tags;

        if start != Position::standard() {
            pgn_set_tag(&mut tags, Key::SetUp, "1");
            pgn_set_tag(&mut tags, Key::Fen, start.fen());
        }

        Self { tags, intro: game.intro.map(Comment), moves, outcome: game.outcome }
    }
}

impl TryFrom<Game> for crate::Game {
    type Error = Error;

    fn try_from(pgn: Game) -> Result<Self> {
        let position = game_position(&pgn.tags)?;
        let mut game = crate::Game::new(position);
        game.tags = pgn.tags;
        game.intro = pgn.intro.map(Into::into);
        game.outcome = pgn.outcome;

        game_moves(&mut game, Node::Start, 0, pgn.moves)?;

        Ok(game)
    }
}

//
// Helper functions for TryFrom<pgn::Game> for crate::Game
//
// The prefix `game_` means crate::Game
//

fn game_position(tags: &[Tag]) -> Result<Position> {
    if let Some(fen) = tags.iter().find(|tag| tag.key == Key::Fen) {
        let unvalidated = fen::position_unvalidated
            .parse(fen.value.as_str())
            .map_err(|_| Error::Fen(fen.value.clone()))?;
        Position::new(unvalidated).map_err(|_| Error::Fen(fen.value.clone()))
    } else {
        Ok(Position::standard())
    }
}

fn game_moves(
    game: &mut crate::Game,
    mut previous: Node,
    mut ply: usize,
    moves: Vec<Move>,
) -> Result<()> {
    for pgn_move in moves {
        let mut options = game.options_mut(previous).expect("previous play exists");
        let play = pgn_move.san.resolve(options.as_ref().legal())?;
        let slot = options.push(play)?;

        {
            let mut play = game.play_mut(slot).expect("inserted play exists");
            play.meta.comment = pgn_move.comment.map(Into::into);
            for annotation in pgn_move.annotations {
                match annotation {
                    Annotation::Nag(nag) => play.meta.nags.push(nag),
                    Annotation::Command(command) => play.meta.commands.push(command),
                }
            }
        }

        for variation in pgn_move.variations {
            game_variation(game, previous, ply, pgn_move.san, variation)?;
        }

        previous = Node::Play(slot);
        ply += 1;
    }

    Ok(())
}

fn game_variation(
    game: &mut crate::Game,
    previous: Node,
    ply: usize,
    after: san::San,
    variation: Variation,
) -> Result<()> {
    let Some((first, rest)) = variation.moves.split_first() else {
        return Err(Error::EmptyVariation { ply, san: after });
    };

    let mut options = game.options_mut(previous).expect("previous play exists");
    let play = first.san.resolve(options.as_ref().legal())?;
    let slot = options.push(play)?;

    {
        let mut play = game.play_mut(slot).expect("inserted play exists");
        play.meta.intro = variation.intro.map(Into::into);
        play.meta.outro = variation.outro.map(Into::into);
        play.meta.comment = first.comment.clone().map(Into::into);
        for annotation in &first.annotations {
            match annotation {
                Annotation::Nag(nag) => play.meta.nags.push(nag.clone()),
                Annotation::Command(command) => play.meta.commands.push(command.clone()),
            }
        }
    }

    for variation in &first.variations {
        game_variation(game, previous, ply, first.san, variation.clone())?;
    }
    game_moves(game, Node::Play(slot), ply + 1, rest.to_vec())
}

//
// Helper functions for From<crate::Game> for pgn::Game
//
// The prefix `pgn_` means pgn::Game
//

fn pgn_set_tag(tags: &mut Vec<Tag>, key: Key, value: impl Into<String>) {
    if let Some(tag) = tags.iter_mut().find(|tag| tag.key == key) {
        tag.value = value.into();
    } else {
        tags.push(Tag { key, value: value.into() });
    }
}

fn pgn_moves<'a>(game: &'a crate::Game, options: game::OptionsRef<'a>) -> Vec<Move> {
    let mut moves = Vec::new();
    let mut options = options;

    while let Some((play, variations)) = options.split_first() {
        moves.push(pgn_move(game, &play, variations));
        options = play.options();
    }

    moves
}

fn pgn_move(game: &crate::Game, play: &game::Play, variations: game::OptionsRef<'_>) -> Move {
    Move {
        san: san::San::from((play.play(), play.short(), play.check())),
        comment: play.meta.comment.clone().map(Comment),
        annotations: pgn_annotations(&play.meta),
        variations: variations.iter().map(|play| pgn_variation(game, play.slot())).collect(),
    }
}

fn pgn_variation(game: &crate::Game, slot: Slot) -> Variation {
    let play = game.play(slot).expect("option must reference an existing play");
    Variation {
        intro: play.meta.intro.clone().map(Comment),
        moves: pgn_moves_from(game, &play),
        outro: play.meta.outro.clone().map(Comment),
    }
}

fn pgn_moves_from<'a>(game: &'a crate::Game, play: &game::PlayRef<'a>) -> Vec<Move> {
    let mut moves = vec![pgn_move_without_variations(play)];
    moves.extend(pgn_moves(game, play.options()));
    moves
}

fn pgn_move_without_variations(play: &game::Play) -> Move {
    Move {
        san: san::San::from((play.play(), play.short(), play.check())),
        comment: play.meta.comment.clone().map(Comment),
        annotations: pgn_annotations(&play.meta),
        variations: Vec::new(),
    }
}

fn pgn_annotations(meta: &game::Meta) -> Vec<Annotation> {
    meta.nags
        .iter()
        .cloned()
        .map(Annotation::Nag)
        .chain(meta.commands.iter().cloned().map(Annotation::Command))
        .collect()
}
