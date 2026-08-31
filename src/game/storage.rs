use crate::{
    formats::uci,
    game::{self, Node, Slot},
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<game::TagPair>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intro: Option<game::Text>,
    #[serde(default, skip_serializing_if = "game::Outcome::is_unknown")]
    pub outcome: game::Outcome,
    pub start: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Slot>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Play {
    pub slot: Slot,
    #[serde(default, skip_serializing_if = "Node::is_start")]
    pub previous: Node,
    #[serde(default, skip_serializing_if = "game::Meta::is_empty")]
    pub meta: game::Meta,
    pub play: uci::Move,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Slot>,
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
            tags: game.tags.clone(),
            intro: game.intro.clone(),
            outcome: game.outcome,
            start: game.start.fen(),
            options: game.tree.start().to_vec(),
        }
    }
}

impl From<&game::Play> for Play {
    fn from(play: &game::Play) -> Self {
        Self {
            slot: play.slot(),
            previous: play.previous(),
            meta: play.meta.clone(),
            play: uci::Move {
                from: play.play.from,
                to: play.play.to,
                promotion: play.play.promotes(),
            },
            options: play.options.clone(),
        }
    }
}
