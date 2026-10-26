use crate::{
    formats::{Text, uci},
    game::{self, MoveSlot, PositionSlot, tag::Other},
    position::Analysis,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Archive {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub games: Vec<Game>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plays: Vec<Move>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Game {
    #[serde(default)]
    pub roster: game::Roster,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Other>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<game::Outcome>,
    pub start: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Analysis>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<MoveSlot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Move {
    pub slot: MoveSlot,
    #[serde(default, skip_serializing_if = "PositionSlot::is_start")]
    pub previous: PositionSlot,
    #[serde(default, skip_serializing_if = "game::Meta::is_empty")]
    pub meta: game::Meta,
    pub play: uci::Move,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Analysis>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<MoveSlot>,
}

impl game::Game {
    pub fn store(&self) -> Archive {
        Archive {
            games: vec![Game::from(self)],
            plays: self.tree.plays().map(Move::from).collect(),
        }
    }

    pub fn load(_archive: Archive) -> game::Result<Self> {
        todo!()
    }
}

impl From<&game::Game> for Game {
    fn from(game: &game::Game) -> Self {
        Self {
            roster: game.roster().clone(),
            tags: game.tags().to_vec(),
            intro: game.intro.clone(),
            outcome: game.outcome(),
            start: game.start().fen(),
            analysis: game.start_options().state().analysis.clone(),
            options: game.tree.start().to_vec(),
        }
    }
}

impl From<&game::Move> for Move {
    fn from(play: &game::Move) -> Self {
        Self {
            slot: play.slot(),
            previous: play.previous(),
            meta: play.meta.clone(),
            play: uci::Move {
                from: play.play.from,
                to: play.play.to,
                promotion: play.play.promotes(),
            },
            analysis: play.state.analysis.clone(),
            options: play.options.plays.clone(),
        }
    }
}
