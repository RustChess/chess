use crate::{
    Player::{self, *},
    Square,
    position::EnPassant,
    square::Rank::{Six, Three},
};

use super::{Bitboard, Board};

/// Board Move API.
impl Board {
    /// Pieces of `attacker` on this board that attack `square`.
    ///
    /// `occupied` is the occupancy view used for line-of-sight and for
    /// filtering attackers. This supports king-safety checks after the other
    /// player moves: removed pieces are ignored, and slider rays use the
    /// post-move blockers.
    ///
    /// This is not a full hypothetical-board query for moves by `attacker`,
    /// because piece roles and players still come from `self`.
    pub const fn attacks_on(
        self,
        square: Square,
        attacker: Player,
        occupied: Bitboard,
    ) -> Bitboard {
        // Work backwards from the target square to find possible source squares.
        // Sliders, knights and kings are symmetric enough for this.
        let straight = square.rook_sight(occupied).intersection(self.rooks_and_queens());
        let diagonal = square.bishop_sight(occupied).intersection(self.bishops_and_queens());
        let knights = square.knight_moves().intersection(self.knights());
        let kings = square.king_moves().intersection(self.kings());

        // Pawns are directional, so find pawn source squares with the opposite
        // player's pawn attacks, then filter down to `attacker` below.
        let pawns = square.pawn_attack_moves(attacker.other()).intersection(self.pawns());

        let attacks = straight.union(diagonal).union(knights).union(kings).union(pawns);

        // The role filters above include both players' pieces. Intersecting
        // with `occupied` removes pieces that are gone in this occupancy view.
        self.player(attacker).intersection(occupied).intersection(attacks)
    }

    #[inline]
    pub const fn attacked_en_passant(
        self,
        en_passant: EnPassant,
        turn: Player,
    ) -> Option<EnPassant> {
        let to = en_passant.square();
        let pawns = self.pawns().intersection(self.player(turn));
        if pawns.intersection(to.pawn_attack_moves(turn.other())).is_empty() {
            None
        } else {
            Some(en_passant)
        }
    }

    #[inline]
    pub const fn effective_en_passant(
        self,
        en_passant: Option<EnPassant>,
        turn: Player,
    ) -> Option<EnPassant> {
        let Some(en_passant) = en_passant else {
            return None;
        };
        let to = en_passant.square();
        let expected = match to.rank() {
            Three => Black,
            Six => White,
            _ => return None,
        };
        if !turn.eq(expected) {
            return None;
        }

        // If say d6 is the en passant square, check that d5 actually contains a pawn.
        let Some(captured) = to.checked_add(turn.other().pawn_push()) else {
            return None;
        };
        let Some(piece) = self.get(captured) else {
            return None;
        };
        if !piece.eq(turn.other().pawn()) {
            return None;
        }

        self.attacked_en_passant(en_passant, turn)
    }

    pub fn king_shields(self, player: Player) -> Bitboard {
        match self.king_of(player) {
            Some(king) => {
                let attacker = self.player(player.other());
                let straight =
                    king.rook_sight(Bitboard::EMPTY).intersection(self.rooks_and_queens());
                let diagonal =
                    king.bishop_sight(Bitboard::EMPTY).intersection(self.bishops_and_queens());
                let snipers = straight.union(diagonal).intersection(attacker);

                let mut shields = Bitboard::EMPTY;
                let mut snipers = snipers;
                while let Some(sniper) = snipers.pop_first() {
                    let blockers = king.between(sniper).intersection(self.occupied());
                    if !blockers.more_than_one() {
                        shields.append(blockers.intersection(self.player(player)));
                    }
                }

                shields
            }
            None => Bitboard::EMPTY,
        }
    }
}
