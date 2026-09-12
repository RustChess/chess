use crate::{
    formats::{Text, uci},
    game::{self, PlayId, PositionId},
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Archive {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub games: Vec<Game>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plays: Vec<Play>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Game {
    #[serde(default)]
    pub roster: game::Roster,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<game::Tag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<Text>,
    #[serde(default, skip_serializing_if = "game::Outcome::is_unknown")]
    pub outcome: game::Outcome,
    pub start: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<game::Evaluation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<PlayId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Play {
    pub id: PlayId,
    #[serde(default, skip_serializing_if = "PositionId::is_start")]
    pub previous: PositionId,
    #[serde(default, skip_serializing_if = "game::Meta::is_empty")]
    pub meta: game::Meta,
    pub play: uci::Move,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<game::Evaluation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<PlayId>,
}

impl game::Outcome {
    pub fn is_unknown(&self) -> bool {
        matches!(self, game::Outcome::Unknown)
    }
}

impl game::Game {
    pub fn store(&self) -> Archive {
        Archive {
            games: vec![Game::from(self)],
            plays: self.tree.plays().map(Play::from).collect(),
        }
    }

    pub fn load(_archive: Archive) -> game::Result<Self> {
        todo!()
    }
}

impl From<&game::Game> for Game {
    fn from(game: &game::Game) -> Self {
        Self {
            roster: game.roster.clone(),
            tags: game.tags.clone(),
            intro: game.intro.clone(),
            outcome: game.outcome,
            start: game.start().fen(),
            evaluation: game.start_options().state().evaluation,
            options: game.tree.start().to_vec(),
        }
    }
}

impl From<&game::Play> for Play {
    fn from(play: &game::Play) -> Self {
        Self {
            id: play.id(),
            previous: play.previous(),
            meta: play.meta.clone(),
            play: uci::Move {
                from: play.play.from,
                to: play.play.to,
                promotion: play.play.promotes(),
            },
            evaluation: play.state.evaluation,
            options: play.options.plays.clone(),
        }
    }
}
