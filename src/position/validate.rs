use super::*;
use variant::Validate;

impl Position<Unvalidated> {
    // Note that this does not "just" validate, but also
    // removes non-effective en passant rights.
    pub fn validate<V: Validate>(self) -> Result<Position<V>> {
        let board = self.board;
        board.validate()?;
        board.validate_kings(self.turn)?;
        board.validate_pawns()?;
        V::validate_castling(self)?;

        Ok(Position {
            board: self.board,
            turn: self.turn,
            castles: self.castles,
            en_passant: self.effective_en_passant(),
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

impl Position<Chess> {
    pub fn validate_castling(position: Position<Unvalidated>) -> Result<()> {
        for player in Player::ALL {
            for side in Side::ALL {
                if !position.castles.has(player, side) {
                    continue;
                }

                let king = player.king();
                let rook = player.rook();
                if position.board.piece_at(Square::new(File::E, player.backrank())) != Some(king)
                    || position.board.piece_at(player.castle_rook_from(side.chess_rook()))
                        != Some(rook)
                {
                    return Err(Error::Castling(player, side));
                }
            }
        }

        Ok(())
    }
}

impl Position<variant::Freestyle> {
    pub fn validate_castling(position: Position<Unvalidated>) -> Result<()> {
        for player in Player::ALL {
            let Some(king) = position.board.king_of(player) else {
                return Err(Error::KingCount(player));
            };
            if king.rank() != player.backrank() {
                return Err(Error::Castling(player, Side::King));
            }

            let queen_rook = position.castles.get(player, Side::Queen);
            let king_rook = position.castles.get(player, Side::King);
            if queen_rook.is_some() && queen_rook == king_rook {
                return Err(Error::Castling(player, Side::King));
            }

            for side in Side::ALL {
                let Some(rook_file) = position.castles.get(player, side) else {
                    continue;
                };

                let valid_side = match side {
                    Side::Queen => rook_file < king.file(),
                    Side::King => king.file() < rook_file,
                };
                let rook = player.rook();
                if !valid_side
                    || position.board.piece_at(player.castle_rook_from(rook_file)) != Some(rook)
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
        formats::{Parser as _, fen::parse_position},
        position::{Castles, Error, File, Player, Position, Side, Square, Square::*},
        variant::{Chess, Freestyle, Unvalidated},
    };

    fn validate(fen: &str) -> Result<Position<Chess>, Error> {
        parse_position.parse(fen).unwrap().validate()
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
    fn drops_ineffective_en_passant() {
        let position = validate("4k3/8/8/8/8/8/8/4K3 w - e3 0 1").unwrap();

        assert_eq!(position.en_passant, None);
    }

    #[test]
    fn rejects_side_not_to_move_in_check() {
        assert_eq!(
            validate("4k3/8/8/8/8/8/4R3/4K3 w - - 0 1").unwrap_err(),
            Error::KingAttacked(Player::Black)
        );
    }

    fn freestyle_position(
        king_file: File,
        queen_rook: File,
        king_rook: File,
    ) -> Position<Unvalidated> {
        use Player::*;

        let mut position = Position::empty();
        position.set_piece(Square::new(king_file, White.backrank()), White.king());
        position.set_piece(Square::new(queen_rook, White.backrank()), White.rook());
        position.set_piece(Square::new(king_rook, White.backrank()), White.rook());
        position.set_piece(Square::new(king_file, Black.backrank()), Black.king());
        position.set_piece(Square::new(queen_rook, Black.backrank()), Black.rook());
        position.set_piece(Square::new(king_rook, Black.backrank()), Black.rook());
        position.castles = Castles::empty();
        position.castles.set(White, Side::Queen, queen_rook);
        position.castles.set(White, Side::King, king_rook);
        position.castles.set(Black, Side::Queen, queen_rook);
        position.castles.set(Black, Side::King, king_rook);
        position
    }

    #[test]
    fn validates_freestyle_castling() {
        freestyle_position(File::C, File::A, File::H).validate::<Freestyle>().unwrap();
    }

    #[test]
    fn rejects_freestyle_castling_without_backrank_king() {
        let mut position = freestyle_position(File::C, File::A, File::H);
        position.move_piece(C1, C2).unwrap();

        assert_eq!(
            position.validate::<Freestyle>().unwrap_err(),
            Error::Castling(Player::White, Side::King)
        );
    }

    #[test]
    fn rejects_freestyle_castling_with_rook_on_wrong_side() {
        assert_eq!(
            freestyle_position(File::C, File::B, File::A).validate::<Freestyle>().unwrap_err(),
            Error::Castling(Player::Black, Side::King)
        );
    }
}
