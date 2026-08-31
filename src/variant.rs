use crate::{
    bitboard::Bitboard,
    position::{Board, Error, File, Piece, Player, Position, Result, Role, Side, Square, Variant},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Chess;

impl Variant for Chess {
    fn validate(position: Position<Unvalidated>) -> Result<Position<Self>> {
        validate_board(position.board)?;
        validate_kings(position.board, position.turn)?;
        validate_pawns(position.board)?;
        validate_castling(position)?;
        // TODO: Validate en-passant plausibility beyond the restricted
        // en_passant::Square rank type.

        Ok(Position {
            board: position.board,
            turn: position.turn,
            castle: position.castle,
            en_passant: position.en_passant,
            reversible: position.reversible,
            round: position.round,
            variant: Chess,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Freestyle;

impl Variant for Freestyle {
    fn validate(_position: Position<Unvalidated>) -> Result<Position<Self>> {
        todo!();
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Unvalidated;

impl Variant for Unvalidated {
    fn validate(position: Position<Unvalidated>) -> Result<Position<Self>> {
        Ok(position)
    }
}

fn validate_board(board: Board) -> Result<()> {
    if !board.players.white.is_disjoint_const(board.players.black) {
        return Err(Error::InconsistentBoard);
    }

    let players = board.players.white.union_const(board.players.black);
    if players != board.occupied {
        return Err(Error::InconsistentBoard);
    }

    let mut roles = Bitboard::EMPTY;
    for role in Role::ALL {
        let squares = board.role(role);
        if !roles.is_disjoint_const(squares) {
            return Err(Error::InconsistentBoard);
        }
        roles.append_const(squares);
    }

    if roles != board.occupied {
        return Err(Error::InconsistentBoard);
    }

    Ok(())
}

fn validate_kings(board: Board, turn: Player) -> Result<()> {
    for player in Player::ALL {
        if board.role(Role::King).intersection_const(board.player(player)).len() != 1 {
            return Err(Error::KingCount(player));
        }
    }

    let white = board.king_of(Player::White).expect("validated king count");
    let black = board.king_of(Player::Black).expect("validated king count");
    if white.king_moves().contains(black) {
        return Err(Error::AdjacentKings);
    }

    let player = turn.other();
    let king = match player {
        Player::Black => black,
        Player::White => white,
    };
    if !board.attacks_on(king, turn, board.occupied()).is_empty() {
        return Err(Error::KingAttacked(player));
    }

    Ok(())
}

fn validate_pawns(board: Board) -> Result<()> {
    if !board.pawns().intersection_const(Bitboard::BACKRANKS).is_empty() {
        return Err(Error::PawnOnBackrank);
    }

    Ok(())
}

fn validate_castling(position: Position<Unvalidated>) -> Result<()> {
    for player in Player::ALL {
        for side in Side::ALL {
            if !position.castle[player][side] {
                continue;
            }

            let king = Piece { player, role: Role::King };
            let rook = Piece { player, role: Role::Rook };
            if position.board.piece_at(Square::new(File::E, player.backrank())) != Some(king)
                || position.board.piece_at(standard_rook(player, side)) != Some(rook)
            {
                return Err(Error::Castling(player, side));
            }
        }
    }

    Ok(())
}

const fn standard_rook(player: Player, side: Side) -> Square {
    let file = match side {
        Side::King => File::H,
        Side::Queen => File::A,
    };
    Square::new(file, player.backrank())
}

#[cfg(test)]
mod tests {
    use crate::{
        formats::{Parser as _, fen::position_unvalidated},
        position::{Error, Position},
        variant::Chess,
    };

    fn validate(fen: &str) -> Result<Position<Chess>, Error> {
        Position::new(position_unvalidated.parse(fen).unwrap())
    }

    #[test]
    fn validates_standard_position() {
        validate("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1").unwrap();
    }

    #[test]
    fn rejects_missing_king() {
        assert_eq!(
            validate("8/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap_err(),
            Error::KingCount(crate::position::Player::Black)
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
            Error::KingAttacked(crate::position::Player::Black)
        );
    }
}
