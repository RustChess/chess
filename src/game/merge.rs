use super::*;

impl Game {
    /// Merges another game with the same starting position, retaining this game's metadata.
    pub fn merge(&mut self, other: &Game) -> Result<()> {
        if self.mode() != other.mode()
            || self.start().position().position() != other.start().position().position()
        {
            return Err(Error::DifferentStart);
        }
        self.merge_position(START, other.start())
    }

    fn merge_position(&mut self, slot: PositionSlot, other: PositionRef<'_>) -> Result<()> {
        let comment = other.comment();
        self.position_mut(slot)?.update_public(|public| merge_text(&mut public.comment, comment));

        for incoming in other.iter() {
            let play = incoming.play;
            let exists = self.position(slot)?.has_option(play);
            let mut position = self.position_mut(slot)?;
            let mut target = position.option_mut_or_push(play)?;
            if exists {
                target.update_public(|public| {
                    merge_text(&mut public.affixes.intro, incoming.public.affixes.intro.as_ref());
                    merge_text(&mut public.affixes.outro, incoming.public.affixes.outro.as_ref());
                });
            } else {
                target.update_public(|public| *public = incoming.public.clone());
            }
            let target_slot = target.slot();
            self.merge_position(target_slot.into(), incoming.position())?;
        }
        Ok(())
    }
}

fn merge_text(target: &mut Option<Text>, incoming: Option<&Text>) {
    if let Some(incoming) = incoming {
        match target {
            Some(existing) if existing != incoming => existing.merge(incoming),
            Some(_) => {}
            None => *target = Some(incoming.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::pgn::Pgn;

    #[test]
    fn merges_moves_and_distinct_comments() {
        let first: Pgn = "1. e4 {First} e5 *".parse().unwrap();
        let second: Pgn = "1. e4 {Second} c5 *".parse().unwrap();
        let mut game = Game::from_pgn(first).unwrap();
        game.merge(&Game::from_pgn(second).unwrap()).unwrap();

        assert_eq!(game.start().iter().count(), 1);
        let e4 = game.start().main().unwrap();
        assert_eq!(e4.comment().unwrap().as_ref(), "First\n\nSecond");
        assert_eq!(e4.position().iter().count(), 2);
    }
}
