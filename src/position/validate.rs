use super::*;
use variant::Validate;

impl Unvalidated {
    pub fn validate<V: Validate>(self) -> Result<Position<V>> {
        let board = self.board;
        board.validate()?;
        board.validate_kings(self.turn)?;
        board.validate_pawns()?;
        V::validate_castling(self)?;
        // TODO: Validate en-passant plausibility beyond the restricted
        // en_passant::Square rank type.

        Ok(Position {
            board: self.board,
            turn: self.turn,
            castles: self.castles,
            en_passant: self.en_passant,
            reversible: self.reversible,
            round: self.round,
            variant: PhantomData,
        })
    }
}

impl Board {
    fn validate(self) -> Result<()> {
        if !self.players.white.is_disjoint_const(self.players.black) {
            return Err(Error::InconsistentBoard);
        }

        let players = self.players.white.union_const(self.players.black);
        if players != self.occupied {
            return Err(Error::InconsistentBoard);
        }

        let mut roles = Bitboard::EMPTY;
        for role in Role::ALL {
            let squares = self.role(role);
            if !roles.is_disjoint_const(squares) {
                return Err(Error::InconsistentBoard);
            }
            roles.append_const(squares);
        }

        if roles != self.occupied {
            return Err(Error::InconsistentBoard);
        }

        Ok(())
    }

    fn validate_kings(self, turn: Player) -> Result<()> {
        for player in Player::ALL {
            if self.role(Role::King).intersection_const(self.player(player)).len() != 1 {
                return Err(Error::KingCount(player));
            }
        }

        let white = self.king_of(Player::White).expect("validated king count");
        let black = self.king_of(Player::Black).expect("validated king count");
        if white.king_moves().contains(black) {
            return Err(Error::AdjacentKings);
        }

        let player = turn.other();
        let king = match player {
            Player::Black => black,
            Player::White => white,
        };
        if !self.attacks_on(king, turn, self.occupied()).is_empty() {
            return Err(Error::KingAttacked(player));
        }

        Ok(())
    }

    fn validate_pawns(self) -> Result<()> {
        if !self.pawns().intersection_const(Bitboard::BACKRANKS).is_empty() {
            return Err(Error::PawnOnBackrank);
        }

        Ok(())
    }
}

impl Position<variant::Chess> {
    pub fn validate_castling(position: Unvalidated) -> Result<()> {
        for player in Player::ALL {
            for side in Side::ALL {
                if !position.castles.has(player, side) {
                    continue;
                }

                let king = Piece { player, role: Role::King };
                let rook = Piece { player, role: Role::Rook };
                if position.board.piece_at(Square::new(File::E, player.backrank())) != Some(king)
                    || position.board.piece_at(Square::chess_rook(player, side)) != Some(rook)
                {
                    return Err(Error::Castling(player, side));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        formats::{Parser as _, fen::position_unvalidated},
        position::{Error, Player, Position},
        variant::Chess,
    };

    fn validate(fen: &str) -> Result<Position<Chess>, Error> {
        position_unvalidated.parse(fen).unwrap().validate()
    }

    #[test]
    fn validates_standard_position() {
        validate("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
    }

    #[test]
    fn rejects_missing_king() {
        assert_eq!(
            validate("8/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap_err(),
            Error::KingCount(Player::Black)
        );
    }

    #[test]
    fn rejects_pawn_on_backrank() {
        assert_eq!(validate("4k3/8/8/8/8/8/8/4K2P w - - 0 1").unwrap_err(), Error::PawnOnBackrank);
    }

    #[test]
    fn rejects_side_not_to_move_in_check() {
        assert_eq!(
            validate("4k3/8/8/8/8/8/4R3/4K3 w - - 0 1").unwrap_err(),
            Error::KingAttacked(Player::Black)
        );
    }
}
