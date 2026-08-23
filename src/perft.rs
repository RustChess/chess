use crate::{
    formats::{Parser as _, fen::position_fen},
    position::{Moves, Position, Result, Variant, variant::Unvalidated},
};

impl Variant for Unvalidated {
    fn validate(position: Position<Unvalidated>) -> Result<Position<Self>> {
        Ok(position)
    }

    fn moves(position: &Position<Self>) -> Moves {
        position.legal_moves()
    }
}

pub(crate) fn perft(position: Position<Unvalidated>, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }

    position
        .legal_moves()
        .into_iter()
        .map(|play| perft(position.apply_unchecked(play), depth - 1))
        .sum()
}

fn position(fen: &str) -> Position<Unvalidated> {
    position_fen.parse(fen).unwrap()
}

#[test]
fn startpos() {
    let position = position("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1");
    println!("{:?}", position.board);
    println!("C1 bishop attacks: {:?}", crate::position::Square::C1.bishop_attacks(position.board.occupied()));
    println!("F1 bishop attacks: {:?}", crate::position::Square::F1.bishop_attacks(position.board.occupied()));
    println!("D1 queen attacks: {:?}", crate::position::Square::D1.queen_attacks(position.board.occupied()));

    assert_eq!(perft(position, 1), 20);
    assert_eq!(perft(position, 2), 400);
    assert_eq!(perft(position, 3), 8902);
}

#[test]
fn kiwipete() {
    let position =
        position("r3k2r/p1ppqpb1/bn2pnp1/2PpP3/1p2P3/2N2N2/PPQPBPPP/R3K2R w KQkq - 0 1");

    assert_eq!(perft(position, 1), 48);
    assert_eq!(perft(position, 2), 2039);
    assert_eq!(perft(position, 3), 97862);
}
