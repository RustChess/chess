use std::{
    collections::BTreeMap as Map,
    ops::{Deref, DerefMut},
};

use crate::{
    Move, Position,
    formats::san::Check,
    position::{File, Rank},
};

pub type Id = String;
pub type Duplicate = usize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("move already exists at index {0}")]
    Duplicate(Duplicate),
    #[error("illegal move")]
    Illegal,
    #[error("index {index} out of bounds for length {len}")]
    OutOfBounds { index: usize, len: usize },
}

pub fn id() -> Id {
    const LETTERS: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut id = String::with_capacity(12);
    for _ in 0..12 {
        let index = rand::random::<u64>() as usize % LETTERS.len();
        id.push(LETTERS[index] as char);
    }
    id
}

#[derive(Clone)]
pub struct Game {
    pub id: Id,
    pub tags: Vec<TagPair>,
    pub intro: Option<Text>,
    /// The starting position of the game.
    start: Position,
    /// The (initial) lines of the game, each containing a sequence of moves,
    /// with recursively nested lines after each move.
    lines: Vec<Id>,
    /// The "arena" in which plays are stored.
    slots: Map<Id, Play>,
}

#[derive(Clone)]
pub struct Play {
    id: Id,
    previous: Option<Id>,
    pub meta: Meta,
    position: Position,
    play: Move,
    short: Short,
    check: Option<Check>,
    lines: Vec<Id>,
}

#[derive(Clone, Default)]
pub struct Meta {
    pub intro: Option<Text>,
    pub comment: Option<Text>,
    pub outro: Option<Text>,
    pub nags: Vec<Nag>,
    pub commands: Vec<Command>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Short {
    pub file: Option<File>,
    pub rank: Option<Rank>,
}

impl Short {
    pub fn new(legal: &[Move], play: Move) -> Self {
        let mut short = Short::default();

        if play.role == crate::position::Role::Pawn || play.is_castle() {
            return short;
        }

        let different_move = |other: &Move| *other != play;
        let same_role_and_to = |other: &Move| (other.role, other.to) == (play.role, play.to);
        let ambiguity = legal
            .iter()
            .copied()
            .filter(different_move)
            .filter(same_role_and_to)
            .fold(Ambiguity::default(), |ambiguity, other| ambiguity.consider(play, other));

        if ambiguity.file_resolves() {
            short.file = Some(play.from.file());
        } else if ambiguity.rank_resolves() {
            short.rank = Some(play.from.rank());
        } else if ambiguity.exists {
            short.file = Some(play.from.file());
            short.rank = Some(play.from.rank());
        }

        short
    }
}

#[derive(Default)]
struct Ambiguity {
    exists: bool,
    file: bool,
    rank: bool,
}

impl Ambiguity {
    fn consider(mut self, play: Move, other: Move) -> Self {
        self.exists = true;
        self.file |= other.from.file() == play.from.file();
        self.rank |= other.from.rank() == play.from.rank();
        self
    }

    fn file_resolves(&self) -> bool {
        self.exists && !self.file
    }

    fn rank_resolves(&self) -> bool {
        self.exists && !self.rank
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagPair {
    pub name: Tag,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tag {
    Event,
    Site,
    Date,
    Round,
    White,
    Black,
    Result,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text(String);

impl Text {
    pub fn new(text: impl AsRef<str>) -> Option<Self> {
        let text = text.as_ref().trim();
        (!text.is_empty()).then(|| Self(text.to_string()))
    }

    pub fn merge(&mut self, text: &Text) {
        self.0.push_str("\n\n");
        self.0.push_str(&text.0);
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for Text {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub command: String,
    pub parameters: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Nag {
    Numeric(u32),
    Symbol(String),
}

impl Game {
    pub fn new(position: Position) -> Self {
        Self {
            id: id(),
            tags: Default::default(),
            intro: None,
            start: position,
            lines: Default::default(),
            slots: Default::default(),
        }
    }

    pub fn start(&self) -> Position {
        self.start
    }

    pub fn play(&self, id: Id) -> Option<PlayRef<'_>> {
        self.slots.contains_key(&id).then_some(PlayRef { game: self, id })
    }

    pub fn play_mut(&mut self, id: Id) -> Option<PlayMut<'_>> {
        self.slots.contains_key(&id).then_some(PlayMut { game: self, id })
    }

    pub fn lines(&self) -> LinesRef<'_> {
        self.lines_ref(None)
    }

    pub fn lines_mut(&mut self) -> LinesMut<'_> {
        LinesMut { game: self, id: None }
    }

    fn lines_ref(&self, id: Option<Id>) -> LinesRef<'_> {
        let lines = self.lines_slice(id.as_ref());
        LinesRef { game: self, id, lines }
    }

    fn lines_slice(&self, id: Option<&Id>) -> &[Id] {
        match id {
            None => &self.lines,
            Some(id) => &self.slots.get(id).expect("id exists").lines,
        }
    }

    fn lines_mut_raw(&mut self, id: Option<&Id>) -> &mut Vec<Id> {
        match id {
            None => &mut self.lines,
            Some(id) => &mut self.slots.get_mut(id).expect("id exists").lines,
        }
    }

    pub(crate) fn push_line(&mut self, id: Option<Id>, play: Move) -> Result<Id, Error> {
        let play = self.new_detached_play(id.clone(), play)?;
        self.lines_mut_raw(id.as_ref()).push(play.id.clone());
        Ok(play.id)
    }

    pub(crate) fn insert_line(
        &mut self,
        id: Option<Id>,
        index: usize,
        play: Move,
    ) -> Result<Id, Error> {
        let len = self.lines_ref(id.clone()).len();
        if index > len {
            return Err(Error::OutOfBounds { index, len });
        }
        let play = self.new_detached_play(id.clone(), play)?;
        self.lines_mut_raw(id.as_ref()).insert(index, play.id.clone());
        Ok(play.id)
    }

    fn new_detached_play(&mut self, id: Option<Id>, play: Move) -> Result<Play, Error> {
        // avoid move generation
        if let Some(index) = self.lines_ref(id.clone()).position(play) {
            return Err(Error::Duplicate(index));
        }

        let (previous_position, check) = self.previous_position(id.as_ref());
        if let Some(Check::Checkmate) = check {
            return Err(Error::Illegal);
        }

        // legality check (implies movegen)
        let legal = previous_position.legal_moves();
        if !legal.contains(&play) {
            return Err(Error::Illegal);
        }

        // apply
        let position = previous_position.apply_unchecked(play);

        // update derived state
        let check = if position.is_check() {
            Some(if position.legal_moves().is_empty() { Check::Checkmate } else { Check::Check })
        } else {
            None
        };
        let short = Short::new(&legal, play);

        // create the (detached) play in the arena
        let play = Play::new(id.clone(), play, short, check, position);
        self.slots.insert(play.id.clone(), play.clone());

        Ok(play)
    }

    fn previous_position(&self, id: Option<&Id>) -> (Position, Option<Check>) {
        match id {
            None => (self.start, None),
            Some(id) => {
                let play = self.slots.get(id).expect("id exists");
                (play.position, play.check)
            }
        }
    }

    fn delete_play(&mut self, id: &Id) -> Play {
        let play = self.slots.remove(id).expect("play exists");
        for line in &play.lines {
            self.delete_play(line);
        }
        play
    }
}

impl Play {
    fn new(
        previous: Option<Id>,
        play: Move,
        short: Short,
        check: Option<Check>,
        position: Position,
    ) -> Self {
        Self {
            id: id(),
            previous,
            meta: Default::default(),
            position,
            play,
            short,
            check,
            lines: Default::default(),
        }
    }

    pub fn id(&self) -> &Id {
        &self.id
    }

    pub fn previous(&self) -> Option<&Id> {
        self.previous.as_ref()
    }

    pub fn play(&self) -> Move {
        self.play
    }

    pub fn short(&self) -> Short {
        self.short
    }

    pub fn check(&self) -> Option<Check> {
        self.check
    }

    pub fn position(&self) -> Position {
        self.position
    }
}

pub struct PlayRef<'a> {
    game: &'a Game,
    id: Id,
}

impl Deref for PlayRef<'_> {
    type Target = Play;

    fn deref(&self) -> &Play {
        self.game.slots.get(&self.id).expect("valid play")
    }
}

impl<'a> PlayRef<'a> {
    pub fn lines(&self) -> LinesRef<'_> {
        self.game.lines_ref(Some(self.id.clone()))
    }
}

pub struct PlayMut<'a> {
    game: &'a mut Game,
    id: Id,
}

impl Deref for PlayMut<'_> {
    type Target = Play;

    fn deref(&self) -> &Play {
        self.game.slots.get(&self.id).expect("valid play")
    }
}

impl DerefMut for PlayMut<'_> {
    fn deref_mut(&mut self) -> &mut Play {
        self.game.slots.get_mut(&self.id).expect("valid play")
    }
}

impl PlayMut<'_> {
    pub fn lines(&self) -> LinesRef<'_> {
        self.game.lines_ref(Some(self.id.clone()))
    }

    pub fn lines_mut(&mut self) -> LinesMut<'_> {
        LinesMut { game: self.game, id: Some(self.id.clone()) }
    }
}

#[derive(Clone)]
pub struct LinesRef<'a> {
    game: &'a Game,
    #[allow(dead_code)]
    id: Option<Id>,
    lines: &'a [Id],
}

impl LinesRef<'_> {
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn position(&self, play: Move) -> Option<usize> {
        self.values().position(|option| option.play == play)
    }

    pub fn contains(&self, play: Move) -> bool {
        self.position(play).is_some()
    }

    pub fn lines(&self) -> &[Id] {
        self.lines
    }

    pub fn get(&self, index: usize) -> Option<&Play> {
        let id = self.lines.get(index)?;
        Some(self.game.slots.get(id).expect("line must reference an existing play"))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Id> {
        self.lines().iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &Play> {
        self.iter().map(|(_, play)| play)
    }

    pub fn iter(&self) -> LinesIter<'_> {
        LinesIter { game: self.game, lines: self.lines, index: 0 }
    }
}

impl<'a> IntoIterator for LinesRef<'a> {
    type Item = (&'a Id, &'a Play);
    type IntoIter = LinesIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        LinesIter { game: self.game, lines: self.lines, index: 0 }
    }
}

pub struct LinesIter<'a> {
    game: &'a Game,
    lines: &'a [Id],
    index: usize,
}

impl<'a> Iterator for LinesIter<'a> {
    type Item = (&'a Id, &'a Play);

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.lines.get(self.index)?;
        self.index += 1;
        Some((id, self.game.slots.get(id).expect("line must reference an existing play")))
    }
}

pub struct LinesMut<'a> {
    game: &'a mut Game,
    id: Option<Id>,
}

impl LinesMut<'_> {
    pub fn as_ref(&self) -> LinesRef<'_> {
        self.game.lines_ref(self.id.clone())
    }

    pub fn lines(&self) -> &[Id] {
        self.game.lines_slice(self.id.as_ref())
    }

    fn lines_mut_raw(&mut self) -> &mut Vec<Id> {
        self.game.lines_mut_raw(self.id.as_ref())
    }

    pub fn push(&mut self, play: Move) -> Result<Id, Error> {
        self.game.push_line(self.id.clone(), play)
    }

    pub fn insert(&mut self, index: usize, play: Move) -> Result<Id, Error> {
        self.game.insert_line(self.id.clone(), index, play)
    }

    pub fn take(&mut self, index: usize) -> Option<Play> {
        let id = self.lines_mut_raw().get(index)?.clone();
        self.lines_mut_raw().remove(index);
        Some(self.game.delete_play(&id))
    }

    #[must_use]
    pub fn swap(&mut self, a: usize, b: usize) -> bool {
        let len = self.lines().len();
        if a >= len || b >= len {
            false
        } else {
            self.lines_mut_raw().swap(a, b);
            true
        }
    }

    #[must_use]
    pub fn raise(&mut self, index: usize) -> bool {
        if index >= self.lines().len() { false } else { index == 0 || self.swap(index - 1, index) }
    }

    #[must_use]
    pub fn lower(&mut self, index: usize) -> bool {
        let len = self.lines().len();
        if index >= len { false } else { index + 1 == len || self.swap(index, index + 1) }
    }

    #[must_use]
    pub fn promote(&mut self, index: usize) -> bool {
        let len = self.lines().len();
        if index >= len {
            return false;
        }

        let lines = self.lines_mut_raw();
        let line = lines.remove(index);
        lines.insert(0, line);
        true
    }

    // DANGEROUS, don't want such a thing.
    // For reads, can do self.as_ref().iter() and friends
    // pub fn for_each_mut(&mut self, mut f: impl FnMut(&mut Play)) {
    //     let ids = self.ids().to_vec();

    //     for id in ids {
    //         f(self.game.slots.get_mut(&id).expect("option id must reference an existing play"));
    //     }
    // }
}
