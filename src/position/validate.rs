use crate::{
    Player,
    board::{Bitboard, Players},
};

use super::{Error, Parts, Position, Result, Side};

use Player::*;

impl Position {
    pub fn new(parts: Parts) -> Result<Self> {
        let Parts { board, turn, castles, en_passant, reversible, round } = parts;

        // 1. Exactly one king for each player
        let Some(white) = board.unique_king_of(White) else {
            return Err(Error::KingCount(White));
        };
        let Some(black) = board.unique_king_of(Black) else {
            return Err(Error::KingCount(Black));
        };
        let kings = Players { black, white };

        // 2. Kings not adjacent
        if white.king_moves().contains(black) {
            return Err(Error::AdjacentKings);
        }

        // 3. The player who just moved is not in check
        let other = turn.other();
        let king = kings.get(other);
        if !board.attacks_on(king, turn, board.occupied()).is_empty() {
            return Err(Error::KingAttacked(other));
        }

        // 4. No pawns are on either back rank
        if !board.pawns().intersection(Bitboard::BACKRANKS).is_empty() {
            return Err(Error::PawnOnBackrank);
        }

        for player in Player::ALL {
            if !castles.has(player, Side::Queen) && !castles.has(player, Side::King) {
                continue;
            }

            // 5. A king with castling rights is on its back rank
            let king = kings.get(player);
            if king.rank() != player.backrank() {
                return Err(Error::CastleKing(player));
            }

            for side in Side::ALL {
                let Some(rook_file) = castles.get(player, side) else {
                    continue;
                };
                // 6. Each castle right has player's rook on the indicated square
                if board.get(player.castle_rook_from(rook_file)) != Some(player.rook()) {
                    return Err(Error::CastleRook { player, side, file: rook_file });
                }
                // 7. This castling rook is on the indicated side of the king
                if !Side::of_rook(king, rook_file).eq(side) {
                    return Err(Error::CastleSide { player, side, file: rook_file });
                }
            }
        }

        let en_passant = board.effective_en_passant(en_passant, turn);
        Ok(Position { board, turn, castles, en_passant, reversible, round })
    }
}

impl Parts {
    pub fn validate(self) -> Result<Position> {
        Position::new(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Player, Position, Side, Square,
        formats::{Parser as _, fen::parse_position},
        position::{Error, Parts},
        square::{File, Square::*},
    };

    use File::*;
    use Player::*;

    fn validate(fen: &str) -> Result<Position, Error> {
        Position::new(parse_position.parse(fen).unwrap())
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

    fn freestyle_position(king_file: File, queen_rook: File, king_rook: File) -> Parts {
        let mut parts = Parts::empty();
        let board = &mut parts.board;
        board.insert(Square::new(king_file, White.backrank()), White.king());
        board.insert(Square::new(queen_rook, White.backrank()), White.rook());
        board.insert(Square::new(king_rook, White.backrank()), White.rook());
        board.insert(Square::new(king_file, Black.backrank()), Black.king());
        board.insert(Square::new(queen_rook, Black.backrank()), Black.rook());
        board.insert(Square::new(king_rook, Black.backrank()), Black.rook());

        let castles = &mut parts.castles;
        castles.set(White, Side::Queen, queen_rook);
        castles.set(White, Side::King, king_rook);
        castles.set(Black, Side::Queen, queen_rook);
        castles.set(Black, Side::King, king_rook);
        parts
    }

    #[test]
    fn validates_freestyle_castling() {
        Position::new(freestyle_position(C, A, H)).unwrap();
    }

    #[test]
    fn validates_freestyle_position_without_castling_after_king_moved() {
        parse_position
            .parse("2r5/1pb2rkp/6p1/3p1p2/3P1P2/n1PB1R2/P5PP/3RB1K1 w - - 4 26")
            .map(Position::new)
            .unwrap()
            .unwrap();
    }

    #[test]
    fn rejects_freestyle_castling_without_backrank_king() {
        let mut position = freestyle_position(C, A, H);
        let king = position.board.remove(C1).unwrap();
        position.board.insert(C2, king);

        assert_eq!(Position::new(position).unwrap_err(), Error::CastleKing(White));
    }

    #[test]
    fn rejects_freestyle_castling_with_rook_on_wrong_side() {
        assert_eq!(
            Position::new(freestyle_position(C, B, A)).unwrap_err(),
            Error::CastleSide { player: Black, side: Side::King, file: A }
        );
    }
}
