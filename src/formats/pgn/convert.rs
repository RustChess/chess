use crate::game::{self, Node, Roster};
use crate::variant::{self, Supported, Variant};
use crate::{
    board::scharnagl_by_id,
    position::{self, Chess, Freestyle, Position, SupportedEnum, Unvalidated},
};

use super::*;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Resolve(#[from] Resolve),
    #[error(transparent)]
    Downgrade(#[from] Downgrade),
}

#[derive(Debug, thiserror::Error)]
pub enum Downgrade {
    #[error("invalid PGN start position {fen}: {error}")]
    Start { variant: Option<SupportedEnum>, fen: String, error: position::Error },
    #[error("unsupported PGN variant: {0}")]
    Variant(String),
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

impl<V: Supported> From<crate::Game<V>> for Game {
    fn from(game: crate::Game<V>) -> Self {
        let freestyle = V::is_freestyle();
        let start = game.start();
        let moves = pgn_moves(&game, game.start_options());
        let mut tags = pgn_roster(&game.roster, game.outcome);
        if freestyle {
            tags.push(Tag::freestyle());
        }
        tags.extend(game.tags.into_iter().map(Tag::Other));

        if freestyle || (start.unvalidated() != Position::start().unvalidated()) {
            pgn_set_start(&mut tags, start.unvalidated());
        }
        if freestyle && let Some(id) = scharnagl_by_id(start.board().standard_id()) {
            tags.push(Tag::Chess960Id(id));
        }

        Self {
            tags,
            start: start.unvalidated(),
            intro: game.intro.map(Comment),
            moves,
            outcome: game.outcome,
        }
    }
}

impl<V: Variant> TryFrom<Game> for crate::Game<V> {
    type Error = Error;

    fn try_from(pgn: Game) -> Result<Self> {
        let position = V::validate(pgn.start).map_err(|error| Downgrade::Start {
            variant: V::VARIANT.supported(),
            fen: pgn.start.fen(),
            error,
        })?;
        Ok(game_from_position(pgn, position)?)
    }
}

impl TryFrom<Game> for variant::Game<Downgrade> {
    type Error = Resolve;

    fn try_from(pgn: Game) -> core::result::Result<Self, Resolve> {
        let downgrade = match pgn.supported() {
            Ok(SupportedEnum::Chess) => match Chess::validate(pgn.start) {
                Ok(position) => return game_from_position(pgn, position).map(variant::Game::Chess),
                Err(error) => Downgrade::Start {
                    variant: Some(SupportedEnum::Chess),
                    fen: pgn.start.fen(),
                    error,
                },
            },
            Ok(SupportedEnum::Freestyle) => match Freestyle::validate(pgn.start) {
                Ok(position) => {
                    return game_from_position(pgn, position).map(variant::Game::Freestyle);
                }
                Err(error) => Downgrade::Start {
                    variant: Some(SupportedEnum::Freestyle),
                    fen: pgn.start.fen(),
                    error,
                },
            },
            Err(variant) => Downgrade::Variant(variant),
        };

        let start = pgn.start;
        let game = game_from_position(pgn, start)?;
        Ok(variant::Game::Unvalidated { game, error: downgrade })
    }
}

impl<V: Variant> crate::Game<V> {
    pub fn from_pgn(pgn: Game) -> Result<Self> {
        pgn.try_into()
    }
}

impl Chess {
    pub fn from_pgn(pgn: Game) -> Result<crate::Game<Self>> {
        pgn.try_into()
    }
}

impl Freestyle {
    pub fn from_pgn(pgn: Game) -> Result<crate::Game<Self>> {
        pgn.try_into()
    }
}

impl Unvalidated {
    pub fn from_pgn(pgn: Game) -> Result<crate::Game<Self>> {
        pgn.try_into()
    }
}

fn game_from_position<V: Variant>(
    pgn: Game,
    position: Position<V>,
) -> core::result::Result<crate::Game<V>, Resolve> {
    let mut game = crate::Game::new(position);
    game.roster = game_roster(&pgn.tags);
    game.tags = game_tags(&pgn.tags);
    game.intro = pgn.intro.map(Into::into);
    game.outcome = pgn.outcome;

    game_moves(&mut game, Node::Start, 0, pgn.moves)?;

    Ok(game)
}

fn game_moves<V: Variant>(
    game: &mut crate::Game<V>,
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

fn game_variation<V: Variant>(
    game: &mut crate::Game<V>,
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

fn pgn_set_start(tags: &mut Vec<Tag>, position: Position<Unvalidated>) {
    tags.push(Tag::SetUp(true));
    tags.push(Tag::Fen(position));
}

fn pgn_moves<'a, V>(game: &'a crate::Game<V>, options: game::OptionsRef<'a, V>) -> Vec<Move> {
    let mut moves = Vec::new();
    let mut options = options;

    while let Some((play, variations)) = options.split_first() {
        moves.push(pgn_move(game, &play, variations));
        options = play.options();
    }

    moves
}

fn pgn_move<V>(
    game: &crate::Game<V>,
    play: &game::Play<V>,
    variations: game::OptionsRef<'_, V>,
) -> Move {
    Move {
        san: san::San::from((play.play(), play.short(), play.check())),
        comment: play.meta.comment.clone().map(Comment),
        annotations: pgn_annotations(&play.meta),
        variations: variations.iter().map(|play| pgn_variation(game, play.slot())).collect(),
    }
}

fn pgn_variation<V>(game: &crate::Game<V>, slot: Slot) -> Variation {
    let play = game.play(slot).expect("option must reference an existing play");
    Variation {
        intro: play.meta.intro.clone().map(Comment),
        moves: pgn_moves_from(game, &play),
        outro: play.meta.outro.clone().map(Comment),
    }
}

fn pgn_moves_from<'a, V>(game: &'a crate::Game<V>, play: &game::PlayRef<'a, V>) -> Vec<Move> {
    let mut moves = vec![pgn_move_without_variations(play)];
    moves.extend(pgn_moves(game, play.options()));
    moves
}

fn pgn_move_without_variations<V>(play: &game::Play<V>) -> Move {
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
