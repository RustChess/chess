use crate::Square;

use super::{Bitboard, Board, Player};

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
