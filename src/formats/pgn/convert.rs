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
    #[error(transparent)]
    San(#[from] san::Apply),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl From<crate::Game> for Pgn {
    fn from(game: crate::Game) -> Self {
        Self::from_game(&game, &Export::default())
    }
}

impl Pgn {
    pub fn from_game(game: &crate::Game, export: &Export) -> Self {
        export.game(game)
    }
}

impl TryFrom<Pgn> for crate::Game {
    type Error = Error;

    fn try_from(pgn: Pgn) -> Result<Self> {
        Self::from_pgn_with(pgn, &Default::default())
    }
}

impl crate::Game {
    pub fn from_pgn(pgn: Pgn) -> Result<Self> {
        pgn.try_into()
    }

    pub fn from_pgn_with(pgn: Pgn, import: &Import) -> Result<Self> {
        import.game(pgn)
    }
}

impl Import {
    fn game(&self, pgn: Pgn) -> Result<crate::Game> {
        let mode = pgn.mode();
        let position = pgn.start.validate().map_err(|error| Error::Start {
            mode,
            fen: pgn.start.fen(),
            error,
        })?;
        Ok(self.game_with_position(pgn, position, mode)?)
    }

    fn game_with_position(
        &self,
        pgn: Pgn,
        position: Position,
        mode: Mode,
    ) -> Result<crate::Game, Resolve> {
        let mut game = if mode.is_freestyle() {
            crate::Game::freestyle(position)
        } else {
            crate::Game::new(position)
        };
        game.set_title(Self::title(&pgn.tags));
        game.update_roster(|roster| *roster = Self::roster(&pgn.tags));
        game.update_tags(|tags| *tags = self.tags(&pgn.tags));
        game.set_outcome(pgn.outcome);
        game.set_orientation(Self::orientation(&pgn.tags));

        let mut start = game.start_mut();
        self.comment(&mut start, pgn.intro);
        let mut terminal = self.plays(start, pgn.moves)?;
        self.comment(&mut terminal, pgn.outro);

        Ok(game)
    }

    fn plays<'g>(
        &self,
        mut position: game::PositionMut<'g>,
        plays: impl IntoIterator<Item = Move>,
    ) -> Result<game::PositionMut<'g>, Resolve> {
        for pgn_play in plays {
            position = self.play(position, pgn_play)?.into_position();
        }

        Ok(position)
    }

    fn play<'g>(
        &self,
        position: game::PositionMut<'g>,
        pgn_play: Move,
    ) -> Result<game::MoveMut<'g>, Resolve> {
        let mut play = position.into_push_san(&pgn_play.san)?;
        self.comment(&mut play.position_mut(), pgn_play.comment);
        self.annotations(&mut play, pgn_play.annotations);

        for variation in pgn_play.variations {
            let outro = self.variation(play.previous_mut(), variation)?;
            self.comment(&mut play.position_mut(), outro);
        }

        Ok(play)
    }

    fn variation(
        &self,
        mut position: game::PositionMut<'_>,
        variation: Variation,
    ) -> Result<Option<Comment>, Resolve> {
        let Variation { intro, moves: plays, outro } = variation;
        let mut plays = plays.into_iter();
        let Some(first) = plays.next() else {
            self.comment(&mut position, intro);
            return Ok(outro);
        };

        let intro = self.affix(&mut position, intro);
        let mut play = self.play(position, first)?;
        play.set_intro(intro);
        let outro = outro.map(|Comment { commands, text }| {
            play.set_outro(text);
            Comment { commands, text: None }
        });
        self.plays(play.into_position(), plays)?;
        Ok(outro)
    }

    fn affix(
        &self,
        position: &mut game::PositionMut<'_>,
        comment: Option<Comment>,
    ) -> Option<Text> {
        let Comment { commands, text } = comment?;
        self.commands(position, commands);
        text
    }

    fn comment(&self, position: &mut game::PositionMut<'_>, comment: Option<Comment>) {
        let Some(Comment { commands, text }) = comment else { return };
        if let Some(text) = text {
            position.update_public(|public| match &mut public.comment {
                Some(comment) => comment.merge(&text),
                None => public.comment = Some(text),
            });
        }
        self.commands(position, commands);
    }

    fn commands(
        &self,
        position: &mut game::PositionMut<'_>,
        commands: impl IntoIterator<Item = Command>,
    ) {
        for command in commands {
            match command {
                Command::ColoredArrows(arrows) => {
                    position.update_arrows(|position| position.extend(arrows.iter().copied()));
                }
                Command::ColoredSquares(markers) => {
                    position.update_markers(|position| position.extend(markers.iter().copied()));
                }
                Command::Evaluation(_) if self.eval == import::Eval::Drop => {}
                Command::Other(command)
                    if self.eval == import::Eval::Drop && command.command.as_ref() == "eval" => {}
                Command::Expanded => position.set_expanded(true),
                command => {
                    let command = command.into_other().expect("handled position commands");
                    position.update_commands(|commands| commands.push(command));
                }
            }
        }
    }

    fn roster(tags: &[Tag]) -> Roster {
        let mut roster = Roster::default();

        for tag in tags {
            match tag {
                Tag::Title(_) => {}
                Tag::Event(value) => roster.event = Self::roster_text(value, "?"),
                Tag::Site(value) => roster.site = Self::roster_text(value, "?"),
                Tag::Date(value) => {
                    roster.date = Self::roster_text(&value.to_string(), "????.??.??")
                }
                Tag::Round(value) => roster.round = Self::roster_text(value, "?"),
                Tag::White(value) => roster.white = Self::roster_text(value, "?"),
                Tag::Black(value) => roster.black = Self::roster_text(value, "?"),
                Tag::Annotator(_)
                | Tag::EndDate(_)
                | Tag::EndTime(_)
                | Tag::EventDate(_)
                | Tag::EventType(_)
                | Tag::EventCategory(_)
                | Tag::EventRounds(_)
                | Tag::EventCountry(_)
                | Tag::Id(_)
                | Tag::WhiteName(_)
                | Tag::WhiteTitle(_)
                | Tag::WhiteElo(_)
                | Tag::WhiteFideId(_)
                | Tag::WhiteTeam(_)
                | Tag::WhiteTeamCountry(_)
                | Tag::BlackName(_)
                | Tag::BlackTitle(_)
                | Tag::BlackElo(_)
                | Tag::BlackFideId(_)
                | Tag::BlackTeam(_)
                | Tag::BlackTeamCountry(_)
                | Tag::Outcome(_)
                | Tag::Rated(_)
                | Tag::Termination(_)
                | Tag::Eco(_)
                | Tag::Fen(_)
                | Tag::SetUp(_)
                | Tag::Time(_)
                | Tag::TimeControl(_)
                | Tag::Timezone(_)
                | Tag::UtcDate(_)
                | Tag::UtcTime(_)
                | Tag::Variant(_)
                | Tag::Chess960Id(_)
                | Tag::Orientation(_)
                | Tag::TimeType(_)
                | Tag::Source(_)
                | Tag::SourceDate(_)
                | Tag::Publisher(_)
                | Tag::Valuation(_)
                | Tag::Other(_) => {}
            }
        }

        roster
    }

    fn title(tags: &[Tag]) -> Option<Text> {
        tags.iter().rev().find_map(|tag| match tag {
            Tag::Title(value) => Text::new(value).ok(),
            _ => None,
        })
    }

    fn orientation(tags: &[Tag]) -> Player {
        tags.iter()
            .rev()
            .find_map(|tag| match tag {
                Tag::Orientation(player) => Some(*player),
                _ => None,
            })
            .unwrap_or(Player::White)
    }

    fn annotations(
        &self,
        play: &mut game::MoveMut<'_>,
        annotations: impl IntoIterator<Item = Annotation>,
    ) {
        for annotation in annotations {
            match annotation {
                Annotation::Nag(nag) => play.update_nags(|nags| nags.push(nag)),
                Annotation::Valuation(evaluation) => match self.eval {
                    import::Eval::Drop => {}
                    import::Eval::Passthrough => play.position_mut().update_commands(|commands| {
                        commands.push(
                            evaluation
                                .command()
                                .into_other()
                                .expect("evaluation is a game command"),
                        )
                    }),
                },
                Annotation::Command(command) => {
                    self.commands(&mut play.position_mut(), [command]);
                }
            }
        }
    }

    fn roster_text(value: &str, default: &str) -> Option<Text> {
        let value = Text::new(value).ok()?;
        (value.as_ref() != default).then_some(value)
    }

    fn tags(&self, tags: &[Tag]) -> Vec<tag::Other> {
        tags.iter()
            .filter_map(|tag| match tag {
                Tag::Other(tag)
                    if self.eval != import::Eval::Drop || tag.key.as_ref() != "Valuation" =>
                {
                    Some(tag.clone())
                }
                Tag::Valuation(evaluation) if self.eval == import::Eval::Passthrough => {
                    Some(tag::Other {
                        key: Text::new("Valuation").expect("tag name is non-empty"),
                        value: evaluation.to_string(),
                    })
                }
                Tag::TimeControl(time_control) => Some(tag::Other {
                    key: Text::new("TimeControl").expect("tag name is non-empty"),
                    value: time_control.to_string(),
                }),
                tag => tag.metadata(),
            })
            .collect()
    }
}

impl Export {
    fn game(&self, game: &crate::Game) -> Pgn {
        let start = game.start();
        let position = start.position();
        let mut intro = self.comment(start, false);
        let (leading, moves) = self.plays(start);
        Self::merge_comment(&mut intro, leading);
        let outcome = game.outcome();
        let tags = self.tags(game, position);

        Pgn { start: position.parts(), tags: tags.into(), intro, moves, outcome, outro: None }
    }

    fn tags(&self, game: &crate::Game, position: Position) -> Vec<Tag> {
        let freestyle = game.mode().is_freestyle();
        let evaluation = (self.eval == export::Eval::Overwrite)
            .then(|| game.start().analysis().and_then(Into::into))
            .flatten();
        let mut tags = Self::roster(game.roster(), game.outcome(), game.orientation());
        if let Some(title) = game.title() {
            tags.insert(0, Tag::Title(title.to_string()));
        }
        if freestyle {
            tags.push(Tag::freestyle());
        }
        tags.extend(
            game.tags()
                .iter()
                .filter(|tag| {
                    tag.key.as_ref() != "Valuation" || self.eval == export::Eval::Passthrough
                })
                .filter_map(|tag| Tag::from_pair(tag.key.clone(), tag.value.clone())),
        );

        if freestyle || (position.parts() != Position::start().parts()) {
            Self::set_start(&mut tags, position.parts());
        }
        if freestyle && let Some(id) = Scharnagl::from_board(position.board()) {
            tags.push(Tag::Chess960Id(id));
        }
        if let Some(evaluation) = evaluation {
            tags.push(Tag::Valuation(evaluation));
        }
        tags
    }

    fn roster(roster: &Roster, outcome: Option<game::Outcome>, orientation: Player) -> Vec<Tag> {
        let mut tags = Vec::new();
        if let Some(value) = &roster.event {
            tags.push(Tag::Event(value.to_string()));
        }
        if let Some(value) = &roster.site {
            tags.push(Tag::Site(value.to_string()));
        }
        if let Some(value) = &roster.date
            && let Ok(value) = value.parse()
        {
            tags.push(Tag::Date(value));
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
        if orientation == Player::Black {
            tags.push(Tag::Orientation(orientation));
        }
        tags
    }

    fn set_start(tags: &mut Vec<Tag>, position: position::Parts) {
        tags.push(Tag::SetUp(true));
        tags.push(Tag::Fen(position));
    }

    fn plays(&self, mut position: game::PositionRef<'_>) -> (Option<Text>, Vec<Move>) {
        let mut intro = None;
        let mut plays: Vec<Move> = Vec::new();

        while let Ok(play) = position.main() {
            let affixes = play.affixes();
            if let Some(text) = affixes.intro.clone() {
                if let Some(previous) = plays.last_mut() {
                    Self::merge_comment(&mut previous.comment, Some(text));
                } else {
                    intro = Some(text);
                }
            }
            let variations = position.alternatives().map(|play| self.variation(play)).collect();
            let mut pgn = self.play(&play, variations);
            Self::merge_comment(&mut pgn.comment, affixes.outro.clone());
            plays.push(pgn);
            position = play.position();
        }

        (intro, plays)
    }

    fn play(&self, play: &game::MoveRef<'_>, variations: Vec<Variation>) -> Move {
        Move {
            san: play.san(),
            comment: self.comment(play.position(), true),
            annotations: self.annotations(play),
            variations,
        }
    }

    fn variation(&self, play: game::MoveRef<'_>) -> Variation {
        let affixes = play.affixes();

        Variation {
            intro: affixes.intro.clone().map(Comment::from_text),
            moves: self.plays_from(&play),
            outro: affixes.outro.clone().map(Comment::from_text),
        }
    }

    fn plays_from(&self, play: &game::MoveRef<'_>) -> Vec<Move> {
        let mut plays = vec![self.play(play, Vec::new())];
        let (intro, following) = self.plays(play.position());
        Self::merge_comment(&mut plays[0].comment, intro);
        plays.extend(following);
        plays
    }

    fn annotations(&self, play: &game::MoveRef<'_>) -> Vec<Annotation> {
        let position = play.position();
        let evaluation = (self.eval == export::Eval::Overwrite)
            .then(|| position.analysis().and_then(Into::into))
            .flatten();
        let mut annotations: Vec<_> = play.nags().iter().cloned().map(Annotation::Nag).collect();
        if let Some(evaluation) = evaluation {
            annotations.push(Annotation::Valuation(evaluation));
        }
        annotations
    }

    fn comment(&self, position: game::PositionRef<'_>, expanded: bool) -> Option<Comment> {
        let mut commands = Commands::default();
        if !position.arrows().is_empty() {
            commands
                .push(Command::ColoredArrows(command::ColoredArrows(position.arrows().to_vec())));
        }
        if !position.markers().is_empty() {
            commands.push(Command::ColoredSquares(command::ColoredSquares(
                position.markers().to_vec(),
            )));
        }
        if expanded && position.expanded() {
            commands.push(Command::Expanded);
        }
        commands.extend(
            position
                .commands()
                .iter()
                .filter(|command| {
                    command.command.as_ref() != "eval" || self.eval == export::Eval::Passthrough
                })
                .cloned()
                .map(Command::Other),
        );
        let comment = Comment { commands, text: position.comment().cloned() };
        (!comment.is_empty()).then_some(comment)
    }

    fn merge_comment(comment: &mut Option<Comment>, text: Option<Text>) {
        let Some(text) = text else { return };
        match comment {
            Some(comment) => match &mut comment.text {
                Some(existing) => existing.merge(&text),
                None => comment.text = Some(text),
            },
            None => *comment = Some(Comment::from_text(text)),
        }
    }
}
