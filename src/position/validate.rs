use core::marker::PhantomData;

use crate::{
    Board, Player, Role, Square,
    board::Bitboard,
    square::File,
    variant::{self, Chess, Unvalidated, Validate},
};

use super::{Error, Position, Result, Side};

use File::*;
use Player::*;
use Role::*;

impl Position<Unvalidated> {
    // Note that this does not "just" validate, but also
    // removes non-effective en passant rights.
    pub fn validate<V: Validate>(self) -> Result<Position<V>> {
        let board = self.board;
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
    fn validate_kings(self, turn: Player) -> Result<()> {
        for player in Player::ALL {
            if self.role(King).intersection(self.player(player)).len() != 1 {
                return Err(Error::KingCount(player));
            }
        }

        let white = self.king_of(White).expect("validated king count");
        let black = self.king_of(Black).expect("validated king count");
        if white.king_moves().contains(black) {
            return Err(Error::AdjacentKings);
        }

        let player = turn.other();
        let king = match player {
            Black => black,
            White => white,
        };
        if !self.attacks_on(king, turn, self.occupied()).is_empty() {
            return Err(Error::KingAttacked(player));
        }

        Ok(())
    }

    fn validate_pawns(self) -> Result<()> {
        if !self.pawns().intersection(Bitboard::BACKRANKS).is_empty() {
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
                if position.board.get(Square::new(E, player.backrank())) != Some(king)
                    || position.board.get(player.castle_rook_from(side.chess_rook())) != Some(rook)
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
            let queen_rook = position.castles.get(player, Side::Queen);
            let king_rook = position.castles.get(player, Side::King);
            if queen_rook.is_none() && king_rook.is_none() {
                continue;
            }

            let Some(king) = position.board.king_of(player) else {
                return Err(Error::KingCount(player));
            };
            if king.rank() != player.backrank() {
                return Err(Error::Castling(player, Side::King));
            }

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
                    || position.board.get(player.castle_rook_from(rook_file)) != Some(rook)
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
        Player, Position, Side, Square,
        formats::{Parser as _, fen::parse_position},
        position::{Castles, Error},
        square::{File, Square::*},
        variant::{Chess, Freestyle, Unvalidated},
    };

    use File::*;
    use Player::*;

    fn validate(fen: &str) -> Result<Position<Chess>, Error> {
        parse_position.parse(fen).unwrap().validate()
    }

    #[test]
    fn validates_standard_position() {
        validate("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
    }

    #[test]
    fn rejects_missing_king() {
        assert_eq!(validate("8/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap_err(), Error::KingCount(Black));
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
            Error::KingAttacked(Black)
        );
    }

    fn freestyle_position(
        king_file: File,
        queen_rook: File,
        king_rook: File,
    ) -> Position<Unvalidated> {
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
        freestyle_position(C, A, H).validate::<Freestyle>().unwrap();
    }

    #[test]
    fn validates_freestyle_position_without_castling_after_king_moved() {
        parse_position
            .parse("2r5/1pb2rkp/6p1/3p1p2/3P1P2/n1PB1R2/P5PP/3RB1K1 w - - 4 26")
            .unwrap()
            .validate::<Freestyle>()
            .unwrap();
    }

    #[test]
    fn rejects_freestyle_castling_without_backrank_king() {
        let mut position = freestyle_position(C, A, H);
        position.move_piece(C1, C2).unwrap();

        assert_eq!(
            position.validate::<Freestyle>().unwrap_err(),
            Error::Castling(White, Side::King)
        );
    }

    #[test]
    fn rejects_freestyle_castling_with_rook_on_wrong_side() {
        assert_eq!(
            freestyle_position(C, B, A).validate::<Freestyle>().unwrap_err(),
            Error::Castling(Black, Side::King)
        );
    }
}
