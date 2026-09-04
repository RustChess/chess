use crate::{Role, Square, board::Bitboard};

use super::{Move, Position, Side};

use Role::*;

type Moves = Vec<Move>;

/// Position Move API.
impl Position {
    pub fn legal_moves(&self) -> Moves {
        if self.is_check() {
            return self.evasion_moves();
        }

        let shields = self.board().king_shields(self.turn());

        let mut moves = self.pseudo_piece_moves();
        moves.retain(|m| self.piece_move_is_safe(*m, shields));
        moves.extend(self.legal_king_moves());
        moves.extend(self.legal_en_passant_moves());
        moves.extend(self.legal_castle_moves());
        moves
    }

    pub fn legal_castle_moves(&self) -> Moves {
        let mut moves = Moves::new();

        for side in Side::ALL {
            if let Some(play) = self.can_castle(side) {
                moves.push(play);
            }
        }

        moves
    }
    fn evasion_moves(&self) -> Moves {
        let checkers = self.checkers();
        let Some(king) = self.board().king_of(self.turn()) else {
            return Moves::new();
        };

        let mut moves = self.legal_king_evasion_moves(king, checkers);
        if checkers.more_than_one() {
            return moves;
        }

        let Some(checker) = checkers.first() else {
            return moves;
        };

        let shields = self.board().king_shields(self.turn());
        let target = king.between(checker).with(checker);
        let mut piece_moves = self.pseudo_piece_moves_to(target);
        piece_moves.retain(|m| self.piece_move_is_safe(*m, shields));
        moves.extend(piece_moves);
        moves.extend(self.legal_en_passant_moves());

        moves
    }

    pub fn pseudo_piece_moves(&self) -> Moves {
        let target = !self.board().player(self.turn());
        self.pseudo_piece_moves_to(target)
    }

    /// Ordinary non-king, non-en-passant, non-castle pseudo moves landing in
    /// `target`.
    ///
    /// In non-check positions `target` is every square not occupied by us. In
    /// single-check evasions it is the checker square plus blocking squares.
    fn pseudo_piece_moves_to(&self, target: Bitboard) -> Moves {
        let mut moves = Moves::new();

        self.pseudo_pawn_moves(target, &mut moves);
        for role in [Knight, Bishop, Rook, Queen] {
            self.pseudo_role_moves(role, target, &mut moves);
        }

        moves
    }

    fn legal_king_evasion_moves(&self, king: Square, checkers: Bitboard) -> Moves {
        let sliders = checkers.intersection(self.board().sliders());
        let mut attacked = Bitboard::EMPTY;
        let mut sliders = sliders;
        while let Some(checker) = sliders.pop_first() {
            // `king_move_is_safe` checks attacks to the destination, but a
            // slider checking the king still controls the ray through the old
            // king square after the king moves away.
            attacked.append(checker.full_ray(king).difference(Bitboard::from_square(checker)));
        }

        let mut moves = Moves::new();
        let target = self.board().player(self.turn()).union(attacked);

        self.pseudo_role_moves(King, !target, &mut moves);
        moves.retain(|m| self.king_move_is_safe(*m));

        moves
    }

    pub fn legal_king_moves(&self) -> Moves {
        let mut moves = Moves::new();
        let target = !self.board().player(self.turn());

        self.pseudo_role_moves(King, target, &mut moves);
        moves.retain(|m| self.king_move_is_safe(*m));

        moves
    }

    fn king_move_is_safe(&self, play: Move) -> bool {
        let occupied = self.board().occupied().difference(Bitboard::from_square(play.from));
        self.board().attacks_on(play.to, self.turn().other(), occupied).is_empty()
    }

    fn king_square_is_safe(&self, square: Square) -> bool {
        self.board().attacks_on(square, self.turn().other(), self.board().occupied()).is_empty()
    }

    fn pseudo_role_moves(&self, role: Role, target: Bitboard, moves: &mut Moves) {
        let occupied = self.board().occupied();
        let mut pieces = self.board().role(role).intersection(self.board().player(self.turn()));

        while let Some(from) = pieces.pop_first() {
            let piece = role.of(self.turn());
            let mut targets = from.attacks(piece, occupied).intersection(target);
            while let Some(to) = targets.pop_first() {
                moves.push(Move::capture(role, from, to, self.board().role_at(to)));
            }
        }
    }

    fn pseudo_pawn_moves(&self, target: Bitboard, moves: &mut Moves) {
        let occupied = self.board().occupied();
        let them = self.board().player(self.turn().other());
        let pawns = self.board().pawns().intersection(self.board().player(self.turn()));
        let empty = !occupied;
        let push = self.turn().pawn_push();
        let double_push = self.turn().pawn_double_push();
        let [left, right] = self.turn().pawn_capture_directions();
        let double_rank = Bitboard::from_rank(self.turn().pawn_double_push_rank());

        let single = pawns.checked_shift(push).intersection(empty);
        let double = single.intersection(double_rank).checked_shift(push).intersection(empty);
        let captures_left = pawns.checked_shift(left).intersection(them);
        let captures_right = pawns.checked_shift(right).intersection(them);

        let mut targets = single.intersection(target);
        while let Some(to) = targets.pop_first() {
            let from = to.checked_add(push.reverse()).expect("valid pawn source");
            moves.extend(Move::pawn(self.turn(), from, to, None));
        }

        let mut targets = double.intersection(target);
        while let Some(to) = targets.pop_first() {
            let from = to.checked_add(double_push.reverse()).expect("valid pawn source");
            moves.push(Move::normal(Pawn, from, to));
        }

        let mut targets = captures_left.intersection(target);
        while let Some(to) = targets.pop_first() {
            let from = to.checked_add(left.reverse()).expect("valid pawn source");
            moves.extend(Move::pawn(self.turn(), from, to, self.board().role_at(to)));
        }

        let mut targets = captures_right.intersection(target);
        while let Some(to) = targets.pop_first() {
            let from = to.checked_add(right.reverse()).expect("valid pawn source");
            moves.extend(Move::pawn(self.turn(), from, to, self.board().role_at(to)));
        }
    }

    fn legal_en_passant_moves(&self) -> Moves {
        let mut moves = Moves::new();

        if let Some(to) = self.en_passant() {
            let to = to.square();

            let mut pawns = self
                .board()
                .pawns()
                .intersection(self.board().player(self.turn()))
                .intersection(to.pawn_attack_moves(self.turn().other()));

            while let Some(from) = pawns.pop_first() {
                let m = Move::en_passant(from, to);
                if self.en_passant_move_is_safe(m) {
                    moves.push(m);
                }
            }
        }

        moves
    }

    fn piece_move_is_safe(&self, play: Move, shields: Bitboard) -> bool {
        // In a legal, not-in-check position, an ordinary piece move can only
        // expose our king by moving a shielding piece off a slider ray.
        if !shields.contains(play.from) {
            return true;
        }

        match self.board().king_of(self.turn()) {
            // A shielding piece remains safe if it stays on the full ray
            // through the king and its original square. This does not need to
            // be only the segment between king and attacker: ordinary move
            // generation cannot move through either piece, and capturing the
            // attacker is safe.
            Some(king) => king.full_ray(play.from).contains(play.to),
            // Invalid positions without a king have no safe legal moves.
            None => false,
        }
    }

    fn en_passant_move_is_safe(&self, play: Move) -> bool {
        let Some(king) = self.board().king_of(self.turn()) else {
            return false;
        };

        let captured = Square::new(play.to.file(), play.from.rank());
        let occupied = self
            .board()
            .occupied()
            .difference(Bitboard::from_square(play.from))
            .difference(Bitboard::from_square(captured))
            .union(Bitboard::from_square(play.to));

        self.board().attacks_on(king, self.turn().other(), occupied).is_empty()
    }

    pub fn can_castle(&self, side: Side) -> Option<Move> {
        let rook_file = self.castles().get(self.turn(), side)?;
        let king_from = self.board().king_of(self.turn())?;
        let empty_path = self.turn().castle_empty_path(king_from, rook_file);
        if !self.board().occupied().intersection(empty_path).is_empty() {
            return None;
        }

        if self
            .turn()
            .castle_king_path(king_from, side)
            .iter()
            .any(|square| !self.king_square_is_safe(square))
        {
            return None;
        }

        Some(Move::castle(self.turn(), king_from, rook_file))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Player::*,
        position::Parts,
        square::{File::*, Square::*},
    };

    use super::{Move, Position, Side};

    fn freestyle_position() -> Position {
        let mut parts = Parts::empty();
        let board = &mut parts.board;
        board.insert(C1, White.king());
        board.insert(A1, White.rook());
        board.insert(H1, White.rook());
        board.insert(C8, Black.king());
        board.insert(A8, Black.rook());
        board.insert(H8, Black.rook());

        let castles = &mut parts.castles;
        castles.set(White, Side::Queen, A);
        castles.set(White, Side::King, H);
        castles.set(Black, Side::Queen, A);
        castles.set(Black, Side::King, H);
        parts.validate().unwrap()
    }

    #[test]
    fn generates_freestyle_castle_moves() {
        let moves = freestyle_position().legal_castle_moves();

        assert!(moves.contains(&Move::castle(White, C1, A)));
        assert!(moves.contains(&Move::castle(White, C1, H)));
    }

    #[test]
    fn blocks_freestyle_castle_move() {
        let mut parts = freestyle_position().parts();
        parts.board.insert(E1, White.knight());
        let position = parts.validate().unwrap();

        assert!(!position.legal_castle_moves().contains(&Move::castle(White, C1, H)));
    }
}
