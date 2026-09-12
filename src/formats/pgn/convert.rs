use crate::{
    Player,
    board::scharnagl_by_id,
    game::{self, Mode, PositionId, Roster},
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

impl From<crate::Game> for Pgn {
    fn from(game: crate::Game) -> Self {
        let mode = game.mode();
        let freestyle = mode.is_freestyle();
        let start = game.start();
        let start_evaluation = game.start_options().state().evaluation;
        let moves = pgn_moves(&game, game.start_options());
        let mut tags = pgn_roster(&game.roster, game.outcome, game.orientation);
        if freestyle {
            tags.push(Tag::freestyle());
        }
        tags.extend(
            game.tags
                .into_iter()
                .filter(|tag| start_evaluation.is_none() || tag.key.as_ref() != "StartEvaluation")
                .map(Tag::Other),
        );

        if freestyle || (start.parts() != Position::start().parts()) {
            pgn_set_start(&mut tags, start.parts());
        }
        if freestyle && let Some(id) = scharnagl_by_id(start.board().standard_id()) {
            tags.push(Tag::Chess960Id(id));
        }
        if let Some(evaluation) = start_evaluation {
            tags.push(Tag::StartEvaluation(Evaluation(evaluation)));
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

impl TryFrom<Pgn> for crate::Game {
    type Error = Error;

    fn try_from(pgn: Pgn) -> Result<Self> {
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
    pub fn from_pgn(pgn: Pgn) -> Result<Self> {
        pgn.try_into()
    }
}

fn game_from_position(
    pgn: Pgn,
    position: Position,
    mode: Mode,
) -> core::result::Result<crate::Game, Resolve> {
    let mut game = crate::Game::new(position, mode);
    game.roster = game_roster(&pgn.tags);
    game.tags = game_tags(&pgn.tags);
    game.intro = pgn.intro.map(Into::into);
    game.outcome = pgn.outcome;
    game.orientation = game_orientation(&pgn.tags);
    let start_evaluation = game_start_evaluation(&pgn.tags);

    game_moves(&mut game, PositionId::Start, 0, pgn.moves)?;

    if let Some(evaluation) = start_evaluation {
        let mut cursor = game.cursor();
        cursor.set_evaluation(Some(evaluation));
        game = cursor.into_game();
    }

    Ok(game)
}

fn game_moves(
    game: &mut crate::Game,
    mut previous: PositionId,
    mut ply: usize,
    moves: Vec<Move>,
) -> core::result::Result<(), Resolve> {
    for pgn_move in moves {
        let mut options = game.options_mut(previous).expect("previous play exists");
        let play = pgn_move.san.resolve(options.as_ref().legal())?;
        let mut play = options.push(play)?;
        let id = play.id();
        play.meta.comment = pgn_move.comment.map(Into::into);
        let (expanded, evaluation) = game_annotations(&mut play.meta, pgn_move.annotations);
        play.set_evaluation(evaluation);
        if let Some(expanded) = expanded {
            play.options_mut().set_expanded(expanded);
        }

        for variation in pgn_move.variations {
            game_variation(game, previous, ply, variation)?;
        }

        previous = PositionId::Play(id);
        ply += 1;
    }

    Ok(())
}

fn game_variation(
    game: &mut crate::Game,
    previous: PositionId,
    ply: usize,
    variation: Variation,
) -> core::result::Result<(), Resolve> {
    let Some((first, rest)) = variation.moves.split_first() else {
        return Ok(());
    };

    let mut options = game.options_mut(previous).expect("previous play exists");
    let play = first.san.resolve(options.as_ref().legal())?;
    let mut play = options.push(play)?;
    let id = play.id();
    play.meta.intro = variation.intro.map(Into::into);
    play.meta.outro = variation.outro.map(Into::into);
    play.meta.comment = first.comment.clone().map(Into::into);
    let (expanded, evaluation) = game_annotations(&mut play.meta, first.annotations.clone());
    play.set_evaluation(evaluation);
    if let Some(expanded) = expanded {
        play.options_mut().set_expanded(expanded);
    }

    for variation in &first.variations {
        game_variation(game, previous, ply, variation.clone())?;
    }
    game_moves(game, PositionId::Play(id), ply + 1, rest.to_vec())
}

//
// Helper functions for From<crate::Game> for Pgn.
//
// The prefix `pgn_` means Pgn.
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
            | Tag::Orientation(_)
            | Tag::StartEvaluation(_)
            | Tag::Other(_) => {}
        }
    }

    roster
}

fn game_orientation(tags: &[Tag]) -> Player {
    tags.iter()
        .rev()
        .find_map(|tag| match tag {
            Tag::Orientation(player) => Some(*player),
            _ => None,
        })
        .unwrap_or(Player::White)
}

fn game_start_evaluation(tags: &[Tag]) -> Option<game::Evaluation> {
    tags.iter().rev().find_map(|tag| match tag {
        Tag::StartEvaluation(Evaluation(evaluation)) => Some(*evaluation),
        _ => None,
    })
}

fn game_annotations(
    meta: &mut game::Meta,
    annotations: impl IntoIterator<Item = Annotation>,
) -> (Option<bool>, Option<game::Evaluation>) {
    let mut expanded = None;
    let mut evaluation = None;
    for annotation in annotations {
        match annotation {
            Annotation::Nag(nag) => meta.nags.push(nag),
            Annotation::Evaluation(Evaluation(value)) => evaluation = Some(value),
            Annotation::Command(command) => {
                if let Some(value) = pgn_expansion(&command) {
                    expanded = Some(value);
                } else {
                    meta.commands.push(command);
                }
            }
        }
    }
    (expanded, evaluation)
}

fn roster_text(value: &str, default: &str) -> Option<Text> {
    let value = Text::new(value).ok()?;
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

fn pgn_roster(roster: &Roster, outcome: game::Outcome, orientation: Player) -> Vec<Tag> {
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
    tags.push(Tag::Orientation(orientation));
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

fn pgn_move(
    game: &crate::Game,
    play: &game::PlayRef<'_>,
    variations: game::OptionsRef<'_>,
) -> Move {
    Move {
        san: san::San::from((play.play(), play.short(), play.check())),
        comment: play.meta.comment.clone().map(Comment),
        annotations: pgn_annotations(
            &play.meta,
            play.state().evaluation,
            play.options().is_expanded(),
        ),
        variations: variations.iter().map(|play| pgn_variation(game, play.id())).collect(),
    }
}

fn pgn_variation(game: &crate::Game, id: PlayId) -> Variation {
    let play = game.play(id).expect("option must reference an existing play");
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

fn pgn_move_without_variations(play: &game::PlayRef<'_>) -> Move {
    Move {
        san: san::San::from((play.play(), play.short(), play.check())),
        comment: play.meta.comment.clone().map(Comment),
        annotations: pgn_annotations(
            &play.meta,
            play.state().evaluation,
            play.options().is_expanded(),
        ),
        variations: Vec::new(),
    }
}

fn pgn_annotations(
    meta: &game::Meta,
    evaluation: Option<game::Evaluation>,
    expanded: bool,
) -> Vec<Annotation> {
    let mut annotations: Vec<_> = meta.nags.iter().cloned().map(Annotation::Nag).collect();
    if let Some(evaluation) = evaluation {
        annotations.push(Annotation::Evaluation(Evaluation(evaluation)));
    }
    if expanded {
        annotations.push(Annotation::Command(pgn_command("cgexp", vec!["true".to_string()])));
    }
    annotations.extend(
        meta.commands
            .iter()
            .filter(|command| evaluation.is_none() || command.command.as_ref() != "eval")
            .cloned()
            .map(Annotation::Command),
    );
    annotations
}

fn pgn_expansion(command: &game::Command) -> Option<bool> {
    if command.command.as_ref() != "cgexp" || command.parameters.len() != 1 {
        return None;
    }

    match command.parameters[0].as_str() {
        "false" => Some(false),
        "true" => Some(true),
        _ => None,
    }
}

fn pgn_command(command: &str, parameters: Vec<String>) -> game::Command {
    game::Command { command: Text::new(command).expect("command name is non-empty"), parameters }
}
