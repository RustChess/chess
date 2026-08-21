use std::{
    collections::BTreeMap as Map,
    ops::{Deref, DerefMut},
};

use crate::{Move, Position};

pub type Comment = String;
pub type Id = String;
pub type Duplicate = usize;

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
    pub meta: Meta,
    pub start: Position,
    options: Vec<Id>,
    slots: Map<Id, Play>,
}

#[derive(Clone)]
pub struct Play {
    id: Id,
    previous: Option<Id>,
    play: Move,
    pub comment: Option<Comment>,
    options: Vec<Id>,
}

#[derive(Clone, Default)]
pub struct Meta {
    pub comment: Option<Comment>,
}

impl Game {
    pub fn new(position: Position) -> Self {
        Self {
            id: id(),
            meta: Default::default(),
            start: position,
            options: Default::default(),
            slots: Default::default(),
        }
    }

    pub fn play(&self, id: Id) -> Option<PlayRef<'_>> {
        self.slots.contains_key(&id).then_some(PlayRef { game: self, id })
    }

    pub fn play_mut(&mut self, id: Id) -> Option<PlayMut<'_>> {
        self.slots.contains_key(&id).then_some(PlayMut { game: self, id })
    }

    pub fn options(&self) -> OptionsRef<'_> {
        self.options_ref(None)
    }

    pub fn options_mut(&mut self) -> OptionsMut<'_> {
        OptionsMut { game: self, id: None }
    }

    fn options_ref(&self, id: Option<Id>) -> OptionsRef<'_> {
        let ids = self.options_slice(id.as_ref());
        OptionsRef { game: self, id, ids }
    }

    fn options_slice(&self, id: Option<&Id>) -> &[Id] {
        match id {
            None => &self.options,
            Some(id) => &self.slots.get(id).expect("id exists").options,
        }
    }

    pub(crate) fn push_option(&mut self, id: Option<Id>, play: Move) -> Result<Id, Duplicate> {
        if let Some(index) = self.options_ref(id.clone()).position(play) {
            return Err(index);
        }

        let option = Play::new(id.clone(), play);
        let option_id = option.id.clone();

        self.slots.insert(option_id.clone(), option);

        match id {
            None => self.options.push(option_id.clone()),
            Some(id) => self.slots.get_mut(&id).expect("id exists").options.push(option_id.clone()),
        }

        Ok(option_id)
    }

    fn delete_play(&mut self, id: &Id) -> Play {
        let play = self.slots.remove(id).expect("play exists");
        for option in &play.options {
            self.delete_play(option);
        }
        play
    }
}

impl Play {
    fn new(previous: Option<Id>, play: Move) -> Self {
        Self { id: id(), previous, play, comment: None, options: Default::default() }
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
    pub fn options(&self) -> OptionsRef<'_> {
        self.game.options_ref(Some(self.id.clone()))
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
    pub fn options(&self) -> OptionsRef<'_> {
        self.game.options_ref(Some(self.id.clone()))
    }

    pub fn options_mut(&mut self) -> OptionsMut<'_> {
        OptionsMut { game: self.game, id: Some(self.id.clone()) }
    }
}

#[derive(Clone)]
pub struct OptionsRef<'a> {
    game: &'a Game,
    #[allow(dead_code)]
    id: Option<Id>,
    ids: &'a [Id],
}

impl OptionsRef<'_> {
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn position(&self, play: Move) -> Option<usize> {
        self.values().position(|option| option.play == play)
    }

    pub fn contains(&self, play: Move) -> bool {
        self.position(play).is_some()
    }

    pub fn ids(&self) -> &[Id] {
        self.ids
    }

    pub fn get(&self, index: usize) -> Option<&Play> {
        let id = self.ids.get(index)?;
        Some(self.game.slots.get(id).expect("option id must reference an existing play"))
    }

    pub fn keys(&self) -> impl Iterator<Item = &Id> {
        self.ids().iter()
    }

    pub fn values(&self) -> impl Iterator<Item = &Play> {
        self.iter().map(|(_, play)| play)
    }

    pub fn iter(&self) -> OptionsIter<'_> {
        OptionsIter { game: self.game, ids: self.ids, index: 0 }
    }
}

impl<'a> IntoIterator for OptionsRef<'a> {
    type Item = (&'a Id, &'a Play);
    type IntoIter = OptionsIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        OptionsIter { game: self.game, ids: self.ids, index: 0 }
    }
}

pub struct OptionsIter<'a> {
    game: &'a Game,
    ids: &'a [Id],
    index: usize,
}

impl<'a> Iterator for OptionsIter<'a> {
    type Item = (&'a Id, &'a Play);

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.ids.get(self.index)?;
        self.index += 1;
        Some((id, self.game.slots.get(id).expect("option id must reference an existing play")))
    }
}

pub struct OptionsMut<'a> {
    game: &'a mut Game,
    id: Option<Id>,
}

impl OptionsMut<'_> {
    pub fn as_ref(&self) -> OptionsRef<'_> {
        self.game.options_ref(self.id.clone())
    }

    pub fn ids(&self) -> &[Id] {
        self.game.options_slice(self.id.as_ref())
    }

    fn ids_mut(&mut self) -> &mut Vec<Id> {
        match &self.id {
            None => &mut self.game.options,
            Some(previous) => {
                &mut self.game.slots.get_mut(previous).expect("previous exists").options
            }
        }
    }

    pub fn push(&mut self, play: Move) -> Result<Id, Duplicate> {
        self.game.push_option(self.id.clone(), play)
    }

    pub fn insert(&mut self, index: usize, play: Move) -> Result<Id, Duplicate> {
        if let Some(index) = self.as_ref().position(play) {
            return Err(index);
        }

        let option = Play::new(self.id.clone(), play);
        let option_id = option.id.clone();
        self.game.slots.insert(option_id.clone(), option);
        self.ids_mut().insert(index, option_id.clone());
        Ok(option_id)
    }

    pub fn delete(&mut self, index: usize) -> Option<Play> {
        let id = self.ids_mut().get(index)?.clone();
        self.ids_mut().remove(index);
        Some(self.game.delete_play(&id))
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.ids_mut().swap(a, b);
    }

    pub fn raise(&mut self, index: usize) {
        if index > 0 {
            self.swap(index - 1, index);
        }
    }

    pub fn lower(&mut self, index: usize) {
        if index + 1 < self.ids().len() {
            self.swap(index, index + 1);
        }
    }

    pub fn promote(&mut self, index: usize) {
        let ids = self.ids_mut();
        let id = ids.remove(index);
        ids.insert(0, id);
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
