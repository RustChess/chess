use core::cmp::Ordering;

use super::{Move, Score, ScoredLine, Vec1};

#[macros::data]
pub struct DepthLine<M = Move> {
    pub depth: u16,
    #[serde(flatten)]
    pub line: ScoredLine<M>,
}

#[macros::data]
pub struct Evaluation<M = Move> {
    pub depth: u16,
    pub knodes: u32,
    pub lines: Vec1<ScoredLine<M>>,
}

#[macros::data]
pub struct Progress<M = Move> {
    pub knodes: u32,
    pub lines: Vec1<DepthLine<M>>,
}

#[derive(
    macros::Arbitrary, Clone, Copy, Debug, macros::Deserialize, Eq, PartialEq, macros::Serialize,
)]
pub struct Shape {
    pub depth: u16,
    pub width: usize,
}

#[macros::data]
pub struct Valuation<M = Move> {
    pub depth: u16,
    pub knodes: u32,
    #[serde(flatten)]
    pub line: ScoredLine<M>,
}

impl<M> Evaluation<M> {
    pub fn bestmove(&self) -> &M {
        self.lines.first().bestmove()
    }

    pub fn improves(&self, other: &Self) -> bool {
        self.shape() > other.shape()
    }

    pub fn score(&self) -> Score {
        self.lines.first().score()
    }

    pub fn shape(&self) -> Shape {
        Shape { depth: self.depth, width: self.lines.len() }
    }
}

impl<M: Clone> TryFrom<&Progress<M>> for Evaluation<M> {
    type Error = ();

    fn try_from(progress: &Progress<M>) -> Result<Self, Self::Error> {
        let depth = progress.lines.first().depth;
        if progress.lines.iter().any(|line| line.depth != depth) {
            return Err(());
        }
        let lines = progress.lines.iter().map(|line| line.line.clone()).collect();
        Ok(Self {
            depth,
            knodes: progress.knodes,
            lines: Vec1::new(lines).expect("progress has lines"),
        })
    }
}

impl<M: Clone> Evaluation<M> {
    pub fn valuation(&self) -> Valuation<M> {
        Valuation { depth: self.depth, knodes: self.knodes, line: self.lines.first().clone() }
    }
}

impl PartialOrd for Shape {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.depth.cmp(&other.depth), self.width.cmp(&other.width)) {
            (Ordering::Equal, Ordering::Equal) => Some(Ordering::Equal),
            (Ordering::Equal | Ordering::Greater, Ordering::Equal | Ordering::Greater) => {
                Some(Ordering::Greater)
            }
            (Ordering::Equal | Ordering::Less, Ordering::Equal | Ordering::Less) => {
                Some(Ordering::Less)
            }
            _ => None,
        }
    }
}

impl<M> Valuation<M> {
    pub fn bestmove(&self) -> &M {
        self.line.bestmove()
    }

    pub fn score(&self) -> Score {
        self.line.score()
    }

    pub const fn shape(&self) -> Shape {
        Shape { depth: self.depth, width: 1 }
    }
}
