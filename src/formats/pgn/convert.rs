use crate::{
    board::scharnagl_by_id,
    game::{self, Mode, Node, Roster},
    position::{self, Position},
};

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Resolve(#[from] Resolve),
    #[error("invalid PGN start position {fen}: {error}")]
    Start { mode: Mode, fen: String, error: position::Error },
}

#[derive(Debug, thiserror::Error)]
// Conceptually, resolving PGN movetext into a game tree has four expected
// failure modes: invalid SAN, illegal SAN, ambiguous SAN, or a duplicate move.
// The source-shaped variants below should be refined with the Game API.
//
// Additionally, our currently san.resolve calculates legal moves twice.
pub enum Resolve {
    #[error("SAN error: {0}")]
    San(#[from] san::Error),
    #[error("game error: {0}")]
    Game(#[from] game::Error),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl From<crate::Game> for Game {
    fn from(game: crate::Game) -> Self {
        let mode = game.mode();
        let freestyle = mode.is_freestyle();
        let start = game.start();
        let moves = pgn_moves(&game, game.start_options());
        let mut tags = pgn_roster(&game.roster, game.outcome);
        if freestyle {
            tags.push(Tag::freestyle());
        }
        tags.extend(game.tags.into_iter().map(Tag::Other));

        if freestyle || (start.parts() != Position::start().parts()) {
            pgn_set_start(&mut tags, start.parts());
        }
        if freestyle && let Some(id) = scharnagl_by_id(start.board().standard_id()) {
            tags.push(Tag::Chess960Id(id));
        }

        Self {
            tags,
            start: start.parts(),
            intro: game.intro.map(Comment),
            moves,
            outcome: game.outcome,
        }
    }
}

impl TryFrom<Game> for crate::Game {
    type Error = Error;

    fn try_from(pgn: Game) -> Result<Self> {
        let mode = pgn.mode();
        let position = pgn.start.validate().map_err(|error| Error::Start {
            mode,
            fen: pgn.start.fen(),
            error,
        })?;
        Ok(game_from_position(pgn, position, mode)?)
    }
}

impl crate::Game {
    pub fn from_pgn(pgn: Game) -> Result<Self> {
        pgn.try_into()
    }
}

fn game_from_position(
    pgn: Game,
    position: Position,
    mode: Mode,
) -> core::result::Result<crate::Game, Resolve> {
    let mut game = crate::Game::new(position, mode);
    game.roster = game_roster(&pgn.tags);
    game.tags = game_tags(&pgn.tags);
    game.intro = pgn.intro.map(Into::into);
    game.outcome = pgn.outcome;

    game_moves(&mut game, Node::Start, 0, pgn.moves)?;

    Ok(game)
}

fn game_moves(
    game: &mut crate::Game,
    mut previous: Node,
    mut ply: usize,
    moves: Vec<Move>,
) -> core::result::Result<(), Resolve> {
    for pgn_move in moves {
        let mut options = game.options_mut(previous).expect("previous play exists");
        let play = pgn_move.san.resolve(options.as_ref().legal())?;
        let mut play = options.push(play)?;
        let slot = play.slot();
        play.meta.comment = pgn_move.comment.map(Into::into);
        for annotation in pgn_move.annotations {
            match annotation {
                Annotation::Nag(nag) => play.meta.nags.push(nag),
                Annotation::Command(command) => play.meta.commands.push(command),
            }
        }

        for variation in pgn_move.variations {
            game_variation(game, previous, ply, variation)?;
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
    variation: Variation,
) -> core::result::Result<(), Resolve> {
    let Some((first, rest)) = variation.moves.split_first() else {
        return Ok(());
    };

    let mut options = game.options_mut(previous).expect("previous play exists");
    let play = first.san.resolve(options.as_ref().legal())?;
    let mut play = options.push(play)?;
    let slot = play.slot();
    play.meta.intro = variation.intro.map(Into::into);
    play.meta.outro = variation.outro.map(Into::into);
    play.meta.comment = first.comment.clone().map(Into::into);
    for annotation in &first.annotations {
        match annotation {
            Annotation::Nag(nag) => play.meta.nags.push(nag.clone()),
            Annotation::Command(command) => play.meta.commands.push(command.clone()),
        }
    }

    for variation in &first.variations {
        game_variation(game, previous, ply, variation.clone())?;
    }
    game_moves(game, Node::Play(slot), ply + 1, rest.to_vec())
}

//
// Helper functions for From<crate::Game> for pgn::Game
//
// The prefix `pgn_` means pgn::Game
//

fn game_roster(tags: &[Tag]) -> Roster {
    let mut roster = Roster::default();

    for tag in tags {
        match tag {
            Tag::Event(value) => roster.event = roster_text(value, "?"),
            Tag::Site(value) => roster.site = roster_text(value, "?"),
            Tag::Date(value) => roster.date = roster_text(value, "????.??.??"),
            Tag::Round(value) => roster.round = roster_text(value, "?"),
            Tag::White(value) => roster.white = roster_text(value, "?"),
            Tag::Black(value) => roster.black = roster_text(value, "?"),
            Tag::Outcome(_)
            | Tag::Fen(_)
            | Tag::SetUp(_)
            | Tag::Variant(_)
            | Tag::Chess960Id(_)
            | Tag::Other(_) => {}
        }
    }

    roster
}

fn roster_text(value: &str, default: &str) -> Option<game::Text> {
    let value = game::Text::new(value)?;
    (value.as_ref() != default).then_some(value)
}

fn game_tags(tags: &[Tag]) -> Vec<game::Tag> {
    tags.iter()
        .filter_map(|tag| match tag {
            Tag::Other(tag) => Some(tag.clone()),
            _ => None,
        })
        .collect()
}

fn pgn_roster(roster: &Roster, outcome: game::Outcome) -> Vec<Tag> {
    let mut tags = Vec::new();
    if let Some(value) = &roster.event {
        tags.push(Tag::Event(value.to_string()));
    }
    if let Some(value) = &roster.site {
        tags.push(Tag::Site(value.to_string()));
    }
    if let Some(value) = &roster.date {
        tags.push(Tag::Date(value.to_string()));
    }
    if let Some(value) = &roster.round {
        tags.push(Tag::Round(value.to_string()));
    }
    if let Some(value) = &roster.white {
        tags.push(Tag::White(value.to_string()));
    }
    if let Some(value) = &roster.black {
        tags.push(Tag::Black(value.to_string()));
    }
    tags.push(Tag::Outcome(outcome));
    tags
}

fn pgn_set_start(tags: &mut Vec<Tag>, position: position::Parts) {
    tags.push(Tag::SetUp(true));
    tags.push(Tag::Fen(position));
}

fn pgn_moves<'g>(game: &'g crate::Game, options: game::OptionsRef<'g>) -> Vec<Move> {
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

fn pgn_moves_from<'g>(game: &'g crate::Game, play: &game::PlayRef<'g>) -> Vec<Move> {
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
