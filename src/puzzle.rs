use core::fmt;

use std::collections::BTreeMap as Map;

use crate::{
    Game, Move, Position,
    formats::{san::San, uci},
};

// pub const SAMPLE: &str = include_str!("../../tmp/lichess_mate_in_2.csv");
pub const SAMPLE: &str = "";

pub fn sample() -> impl Iterator<Item = Puzzle> {
    SAMPLE.lines().map(|line| Puzzle::from_csv(line).expect("valid puzzle in sample"))
}

#[derive(Clone, Debug)]
pub struct Puzzle {
    pub mistake: Option<Move>,
    pub position: Position,
}

impl Puzzle {
    pub fn from_csv(line: &str) -> Option<Self> {
        let (mistake, fen) = line.split_once(',')?;
        let position = Position::from_fen(fen).ok()?;
        let mistake = mistake.parse::<uci::Move>().ok()?.resolve_chess(&position.legal_moves())?;
        Some(Self { mistake: Some(mistake), position })
    }
}

impl Puzzle {
    pub fn start(&self) -> Position {
        match self.mistake {
            Some(mistake) => self.position.apply_unchecked(mistake),
            None => self.position,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Solutions(pub Map<Move, Map<Move, Map<Move, Position>>>);

#[derive(Clone)]
pub struct MateIn2 {
    pub position: Position,
    pub mate_in_1: Map<Move, Position>,
    pub solutions: Solutions,
}

fn is_checkmate(position: Position) -> bool {
    position.is_check() && position.legal_moves().is_empty()
}

fn san(position: Position, play: Move) -> San {
    let mut game = Game::from(position);
    game.start_options_mut().into_push(play).expect("legal solution move").san()
}

impl fmt::Debug for MateIn2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{:?}", self.position.board())?;

        if !self.mate_in_1.is_empty() {
            write!(f, "mate-in-1:")?;
            for play1 in self.mate_in_1.keys() {
                write!(f, " {}", san(self.position, *play1))?;
            }
            writeln!(f)?;
        }

        for (play1, replies) in &self.solutions.0 {
            writeln!(f, "{}", san(self.position, *play1))?;
            let defend = self.position.apply_unchecked(*play1);
            for (defense, mates) in replies {
                writeln!(f, "  {}", san(defend, *defense))?;
                let attack = defend.apply_unchecked(*defense);
                for play2 in mates.keys() {
                    writeln!(f, "    {}", san(attack, *play2))?;
                }
            }
        }

        Ok(())
    }
}

impl MateIn2 {
    pub fn new(puzzle: Puzzle) -> Option<Self> {
        let position = puzzle.start();
        // position.is_mate_in_2().then(Self { position })
        // let legal = position.legal_moves();
        let mut mate_in_1: Map<Move, Position> = Default::default();
        let mut solutions = Solutions::default().0;
        // first move of our solution
        'play1: for play1 in position.legal_moves() {
            let defend = position.apply_unchecked(play1);
            let defenses = defend.legal_moves();

            // already checkmate? problem was a mate-in-1
            // stalemate => bad move, suppress
            if defenses.is_empty() {
                if defend.is_check() {
                    mate_in_1.insert(play1, defend);
                }
                continue;
            }

            let mut replies = Map::new();
            for defense in defenses {
                let attack = defend.apply_unchecked(defense);
                let mut mates = Map::new();
                for play2 in attack.legal_moves() {
                    let outcome = attack.apply_unchecked(play2);
                    if is_checkmate(outcome) {
                        mates.entry(play2).insert_entry(outcome);
                    }
                }

                if mates.is_empty() {
                    continue 'play1;
                }
                replies.insert(defense, mates);
            }

            solutions.insert(play1, replies);
        }

        if solutions.is_empty() {
            return None;
        }

        Some(Self { position, mate_in_1, solutions: Solutions(solutions) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search() {
        let mut count = 0;
        let mut i = 0;
        let total = 807425.0;
        let mut perc = 5.0;
        for position in sample() {
            i += 1;
            if 100.0 * i as f32 / total > perc {
                println!("{perc}% - {i} of 807425");
                perc += 5.0;
            }
            let Some(puzzle) = MateIn2::new(position) else {
                continue;
            };
            if !puzzle.mate_in_1.is_empty() {
                println!("{puzzle:?}");
                count += 1;
                if count > 10 {
                    break;
                }
            }
            // if puzzle.solutions.0.values().any(|defenses| defenses.len() > 1) {
            //     println!("{puzzle:?}");
            //     count += 1;
            //     if count > 10 {
            //         break;
            //     }
            // }
        }
        panic!();
    }
}
