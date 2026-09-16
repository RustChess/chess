use crate::{
    Player, Scharnagl,
    game::{self, Mode, Roster},
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
        let start = game.start();
        let position = start.position();
        let valuation = start.valuation();
        let intro = start.comment().cloned().map(Comment);
        let plays = pgn_plays(start);
        let outcome = game.outcome;
        let tags = pgn_tags(game, position, valuation);

        Self { start: position.parts(), tags, intro, moves: plays, outcome }
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

fn game_from_position(pgn: Pgn, position: Position, mode: Mode) -> Result<crate::Game, Resolve> {
    let mut game = if mode.is_freestyle() {
        crate::Game::freestyle(position)
    } else {
        crate::Game::new(position)
    };
    game.roster = game_roster(&pgn.tags);
    game.tags = game_tags(&pgn.tags);
    game.outcome = pgn.outcome;
    game.orientation = game_orientation(&pgn.tags);

    let mut start = game.start_mut();
    *start.comment_mut() = pgn.intro.map(Into::into);
    start.set_valuation(game_start_valuation(&pgn.tags));
    game_plays(start, pgn.moves)?;

    Ok(game)
}

fn game_plays<'g>(
    mut position: game::PositionMut<'g>,
    plays: impl IntoIterator<Item = Move>,
) -> Result<(), Resolve> {
    for pgn_play in plays {
        position = game_play(position, pgn_play)?.into_position();
    }

    Ok(())
}

fn game_play<'g>(
    position: game::PositionMut<'g>,
    pgn_play: Move,
) -> Result<game::MoveMut<'g>, Resolve> {
    let play = pgn_play.san.resolve(position.legal())?;
    let mut play = position.into_push(play)?;
    *play.comment_mut() = pgn_play.comment.map(Into::into);
    game_annotations(&mut play, pgn_play.annotations);

    for variation in pgn_play.variations {
        game_variation(play.previous_mut(), variation)?;
    }

    Ok(play)
}

fn game_variation(position: game::PositionMut<'_>, variation: Variation) -> Result<(), Resolve> {
    let Variation { intro, moves: plays, outro } = variation;
    let mut plays = plays.into_iter();
    let Some(first) = plays.next() else {
        return Ok(());
    };

    let mut play = game_play(position, first)?;
    *play.affixes_mut() =
        game::Affixes { intro: intro.map(Into::into), outro: outro.map(Into::into) };
    game_plays(play.into_position(), plays)
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
            | Tag::Valuation(_)
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

fn game_start_valuation(tags: &[Tag]) -> Option<game::Valuation> {
    tags.iter().rev().find_map(|tag| match tag {
        Tag::Valuation(Valuation(valuation)) => Some(*valuation),
        _ => None,
    })
}

fn game_annotations(
    play: &mut game::MoveMut<'_>,
    annotations: impl IntoIterator<Item = Annotation>,
) {
    for annotation in annotations {
        match annotation {
            Annotation::Nag(nag) => play.nags_mut().push(nag),
            Annotation::Valuation(Valuation(valuation)) => {
                play.position_mut().set_valuation(Some(valuation));
            }
            Annotation::Command(command) => {
                if command.is_expanded() {
                    *play.position_mut().expanded_mut() = true;
                } else {
                    play.commands_mut().push(command);
                }
            }
        }
    }
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

fn pgn_tags(game: crate::Game, position: Position, valuation: Option<game::Valuation>) -> Vec<Tag> {
    let freestyle = game.mode().is_freestyle();
    let mut tags = pgn_roster(&game.roster, game.outcome, game.orientation);
    if freestyle {
        tags.push(Tag::freestyle());
    }
    tags.extend(
        game.tags
            .into_iter()
            .filter(|tag| valuation.is_none() || tag.key.as_ref() != "Valuation")
            .map(Tag::Other),
    );

    if freestyle || (position.parts() != Position::start().parts()) {
        pgn_set_start(&mut tags, position.parts());
    }
    if freestyle && let Some(id) = Scharnagl::from_board(position.board()) {
        tags.push(Tag::Chess960Id(id));
    }
    if let Some(valuation) = valuation {
        tags.push(Tag::Valuation(Valuation(valuation)));
    }

    tags
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

fn pgn_plays(mut position: game::PositionRef<'_>) -> Vec<Move> {
    let mut plays = Vec::new();

    while let Some(play) = position.main() {
        let variations = position.alternatives().map(pgn_variation).collect();
        plays.push(pgn_play(&play, variations));
        position = play.position();
    }

    plays
}

fn pgn_play(play: &game::MoveRef<'_>, variations: Vec<Variation>) -> Move {
    Move {
        san: play.san(),
        comment: play.comment().cloned().map(Comment),
        annotations: pgn_annotations(play),
        variations,
    }
}

fn pgn_variation(play: game::MoveRef<'_>) -> Variation {
    let affixes = play.affixes();

    Variation {
        intro: affixes.intro.clone().map(Comment),
        moves: pgn_plays_from(&play),
        outro: affixes.outro.clone().map(Comment),
    }
}

fn pgn_plays_from(play: &game::MoveRef<'_>) -> Vec<Move> {
    let mut plays = vec![pgn_play(play, Vec::new())];
    plays.extend(pgn_plays(play.position()));
    plays
}

fn pgn_annotations(play: &game::MoveRef<'_>) -> Vec<Annotation> {
    let position = play.position();
    let valuation = position.valuation();
    let mut annotations: Vec<_> = play.nags().iter().cloned().map(Annotation::Nag).collect();
    if let Some(valuation) = valuation {
        annotations.push(Annotation::Valuation(Valuation(valuation)));
    }
    if position.expanded() {
        annotations.push(Annotation::Command(game::Command::expanded()));
    }
    annotations.extend(
        play.commands()
            .iter()
            .filter(|command| valuation.is_none() || command.command.as_ref() != "eval")
            .cloned()
            .map(Annotation::Command),
    );
    annotations
}
