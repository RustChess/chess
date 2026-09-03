use crate::{
    formats::{Parser as _, fen::parse_position},
    position::{Position, Scharnagl},
    variant::{Chess, Freestyle, Variant},
};

pub(crate) fn perft<V: Variant>(position: Position<V>, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }

    position
        .legal_moves()
        .into_iter()
        .map(|play| perft(position.apply_unchecked(play), depth - 1))
        .sum()
}

fn assert_perft(name: &str, fen: &str, expected: &[(u32, u64)]) {
    let position = parse_position.parse(fen).unwrap().validate::<Chess>().unwrap();
    for &(depth, nodes) in expected {
        assert_eq!(perft(position, depth), nodes, "{name} depth {depth}");
    }
}

fn assert_freestyle_perft(name: &str, fen: &str, expected: &[(u32, u64)]) {
    let position = parse_position.parse(fen).unwrap().validate::<Freestyle>().unwrap();
    for &(depth, nodes) in expected {
        assert_eq!(perft(position, depth), nodes, "{name} depth {depth}");
    }
}

fn assert_position_perft<V: Variant>(name: &str, position: Position<V>, expected: &[(u32, u64)]) {
    for &(depth, nodes) in expected {
        assert_eq!(perft(position, depth), nodes, "{name} depth {depth}");
    }
}

// This corresponds to a divide reporting mode in e.g. Stockfish,
// allowing to see the number of moves split count after first move
fn divide<V: Variant>(position: Position<V>, depth: u32) {
    for play in position.legal_moves() {
        println!("{}: {}", play.uci(), perft(position.apply_unchecked(play), depth - 1));
    }
}

#[test]
fn freestyle_positions() {
    // Selected from Shakmaty's `tests/chess960.perft`:
    // https://github.com/niklasf/shakmaty/blob/master/shakmaty/tests/chess960.perft
    assert_position_perft(
        "chess960 position 518",
        Position::freestyle(Scharnagl::CHESS),
        &[(1, 20), (2, 400), (3, 8902), (4, 197281)],
    );
    assert_freestyle_perft(
        "chess960 position 0",
        "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9",
        &[(1, 21), (2, 528), (3, 12189), (4, 326672)],
    );
    assert_freestyle_perft(
        "chess960 position 1",
        "2nnrbkr/p1qppppp/8/1ppb4/6PP/3PP3/PPP2P2/BQNNRBKR w HEhe - 1 9",
        &[(1, 21), (2, 807), (3, 18002)],
    );
    assert_freestyle_perft(
        "chess960 position 2",
        "b1q1rrkb/pppppppp/3nn3/8/P7/1PPP4/4PPPP/BQNNRKRB w GE - 1 9",
        &[(1, 20), (2, 479), (3, 10471), (4, 273318)],
    );
    assert_freestyle_perft(
        "chess960 position 5",
        "qnbnr1kr/ppp1b1pp/4p3/3p1p2/8/2NPP3/PPP1BPPP/QNB1R1KR w HEhe - 1 9",
        &[(1, 29), (2, 899), (3, 26578)],
    );
    assert_freestyle_perft(
        "chess960 position 7",
        "qbn1brkr/ppp1p1p1/2n4p/3p1p2/P7/6PP/QPPPPP2/1BNNBRKR w HFhf - 0 9",
        &[(1, 25), (2, 635), (3, 17054)],
    );
}

#[test]
#[ignore]
fn deep_freestyle_positions() {
    let position = parse_position
        .parse("b1q1rrkb/pppppppp/3nn3/8/P7/1PPP4/4PPPP/BQNNRKRB w GE - 1 9")
        .unwrap()
        .validate::<Freestyle>()
        .unwrap();
    divide(position, 5);

    assert_freestyle_perft(
        "chess960 position 0",
        "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9",
        &[(5, 8146062), (6, 227689589)],
    );
    assert_freestyle_perft(
        "chess960 position 2",
        "b1q1rrkb/pppppppp/3nn3/8/P7/1PPP4/4PPPP/BQNNRKRB w GE - 1 9",
        &[(5, 6417013), (6, 177654692)],
    );
    assert_freestyle_perft(
        "chess960 position 4",
        "1nbbnrkr/p1p1ppp1/3p4/1p3P1p/3Pq2P/8/PPP1P1P1/QNBBNRKR w HFhf - 0 9",
        &[(5, 34030312), (6, 1250970898)],
    );
    // Passed on 2026-09-02. This is 1.2B leaf nodes and took 128s in release mode.
    // This is the heaviest case
    // assert_freestyle_perft(
    //     "chess960 position 274",
    //     "bnrk1rqb/2pppp1p/3n4/pp4p1/3Q1P2/2N3P1/PPPPP2P/B1RKNR1B w FCfc - 0 9",
    //     &[(5, 107234294), (6, 3651608327)],
    // );
}

#[test]
fn tricky_positions() {
    // Selected from Shakmaty's `tests/tricky.perft`:
    // https://github.com/niklasf/shakmaty/blob/master/shakmaty/tests/tricky.perft
    //
    // The classic positions are also used by Stockfish's perft test script:
    // https://github.com/official-stockfish/Stockfish/blob/master/tests/perft.sh
    assert_perft(
        "startpos",
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        &[(1, 20), (2, 400), (3, 8902)],
    );
    assert_perft(
        "kiwipete",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &[(1, 48), (2, 2039), (3, 97862)],
    );
    assert_perft(
        "position 3",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        &[(1, 14), (2, 191), (3, 2812), (4, 43238)],
    );
    assert_perft(
        "position 4 mirrored",
        "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1",
        &[(1, 6), (2, 264), (3, 9467)],
    );
    assert_perft(
        "position 5",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        &[(1, 44), (2, 1486), (3, 62379)],
    );
    assert_perft(
        "position 6",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        &[(1, 46), (2, 2079), (3, 89890)],
    );
    assert_perft(
        "ep evasion",
        "8/8/8/5k2/3p4/8/4P3/4K3 w - - 0 1",
        &[(1, 6), (2, 54), (3, 343), (4, 2810), (5, 19228)],
    );
    assert_perft(
        "king walk",
        "8/8/8/B2p3Q/2qPp1P1/b7/2P2PkP/4K2R b K - 0 1",
        &[(1, 26), (2, 611), (3, 14583)],
    );
    assert_perft("prison", "2b5/kpPp4/1p1P4/1P6/6p1/4p1P1/4PpPK/5B2 w - - 0 1", &[(1, 1), (2, 1)]);
    assert_perft("align ep", "8/8/8/1k6/3Pp3/8/8/4KQ2 b - d3 0 1", &[(1, 6), (2, 121), (3, 711)]);
    assert_perft(
        "align ep pinned",
        "1b1k4/8/8/1rPpK3/8/8/8/8 w - d6 0 1",
        &[(1, 5), (2, 100), (3, 555)],
    );
    assert_perft(
        "ep unrelated check",
        "rnbqk1nr/bb3p1p/1q2r3/2pPp3/3P4/7P/1PP1NpPP/R1BQKBNR w KQkq c6 0 1",
        &[(1, 2), (2, 92), (3, 2528)],
    );
    assert_perft(
        "two pawn checkers",
        "1rrrrrk1/1PPPPPPP/8/8/8/8/8/6K1 b - - 0 1",
        &[(1, 3), (2, 131), (3, 1919)],
    );
    assert_perft(
        "two stepper checkers",
        "1q4k1/3r1Ppp/5NP1/pP6/8/1Q6/3B4/2K2R2 b - - 0 1",
        &[(1, 2), (2, 98), (3, 2826)],
    );
    assert_perft(
        "two knight checkers",
        // This FEN is actually incorrect - the K castling right is not valid
        // "2b5/1nbn4/n3n3/1kn5/n3n3/1n1n4/5RQ1/2KQ1R2 w K - 0 1",
        "2b5/1nbn4/n3n3/1kn5/n3n3/1n1n4/5RQ1/2KQ1R2 w - - 0 1",
        &[(1, 2), (2, 104), (3, 3382)],
    );
    assert_perft(
        "align diagonal 1",
        "3R4/8/q4k2/2B5/1NK5/3b4/8/8 w - - 0 1",
        &[(1, 4), (2, 125), (3, 2854)],
    );
    assert_perft(
        "align diagonal 2",
        "2Nq4/2K5/1b6/8/7R/3k4/7P/8 w - - 0 1",
        &[(1, 3), (2, 81), (3, 1217)],
    );
    assert_perft(
        "align horizontal",
        "5R2/2P5/8/4k3/8/3rK2r/8/8 w - - 0 1",
        &[(1, 2), (2, 56), (3, 1030)],
    );
    assert_perft(
        "max legals",
        "R6R/3Q4/1Q4Q1/4Q3/2Q4Q/Q4Q2/pp1Q4/kBNN1KB1 w - - 0 1",
        &[(1, 218), (2, 99)],
    );
    // Shakmaty's `asymmetrical-and-king-on-h` regression covers impossible
    // castling rights: black has a castling right even though the king is on
    // h8. Our current movegen assumes validated standard castling positions.
    //
    // assert_perft(
    //     "asymmetrical and king on h",
    //     "r2r3k/p7/3p4/8/8/P6P/8/R3K2R b KQq - 0 1",
    //     &[(1, 14), (2, 206), (3, 3672)],
    // );
}

#[test]
#[ignore]
fn deep_startpos() {
    assert_perft(
        "startpos",
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        &[(6, 119060324)],
    );
}

// Passed on 2026-08-23. This is 3.2B leaf nodes and took 75s in release mode.
// #[test]
// #[ignore]
// fn superdeep_startpos() {
//     assert_perft(
//         "startpos",
//         "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
//         &[(7, 3195901860)],
//     );
// }

#[test]
#[ignore]
fn deep_kiwipete() {
    assert_perft(
        "kiwipete",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        &[(4, 4085603), (5, 193690690)],
    );
}

#[test]
#[ignore]
fn deep_positions_3_to_6() {
    assert_perft(
        "position 3",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        &[(5, 674624), (6, 11030083), (7, 178633661)],
    );
    assert_perft(
        "position 4",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        &[(6, 706045033)],
    );
    assert_perft(
        "position 4 mirrored",
        "r2q1rk1/pP1p2pp/Q4n2/bbp1p3/Np6/1B3NBn/pPPP1PPP/R3K2R b KQ - 0 1",
        &[(4, 422333), (5, 15833292)],
    );
    assert_perft(
        "position 5",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        &[(4, 2103487), (5, 89941194)],
    );
    assert_perft(
        "position 6",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        &[(4, 3894594), (5, 164075551)],
    );
    assert_perft(
        "stockfish extra",
        "r7/4p3/5p1q/3P4/4pQ2/4pP2/6pp/R3K1kr w Q - 1 3",
        &[(5, 11609488)],
    );
}

#[test]
#[ignore]
fn deep_prison() {
    assert_perft("prison", "2b5/kpPp4/1p1P4/1P6/6p1/4p1P1/4PpPK/5B2 w - - 0 1", &[(32, 1)]);
}

#[test]
#[ignore]
fn martin_sedlak_positions() {
    // Martin Sedlak's TalkChess movegen tests, mirrored here:
    // https://www.chessprogramming.net/perfect-perft/
    assert_perft("illegal ep move 1", "3k4/3p4/8/K1P4r/8/8/8/8 b - - 0 1", &[(6, 1134888)]);
    assert_perft("illegal ep move 2", "8/8/4k3/8/2p5/8/B2P2K1/8 w - - 0 1", &[(6, 1015133)]);
    assert_perft(
        "ep capture checks opponent",
        "8/8/1k6/2b5/2pP4/8/5K2/8 b - d3 0 1",
        &[(6, 1440467)],
    );
    assert_perft("short castling gives check", "5k2/8/8/8/8/8/8/4K2R w K - 0 1", &[(6, 661072)]);
    assert_perft("long castling gives check", "3k4/8/8/8/8/8/8/R3K3 w Q - 0 1", &[(6, 803711)]);
    assert_perft("castle rights", "r3k2r/1b4bq/8/8/8/8/7B/R3K2R w KQkq - 0 1", &[(4, 1274206)]);
    assert_perft("castling prevented", "r3k2r/8/3Q4/8/8/5q2/8/R3K2R b KQkq - 0 1", &[(4, 1720476)]);
    assert_perft("promote out of check", "2K2r2/4P3/8/8/8/8/8/3k4 w - - 0 1", &[(6, 3821001)]);
    assert_perft("discovered check", "8/8/1P2K3/8/2n5/1q6/8/5k2 b - - 0 1", &[(5, 1004658)]);
    assert_perft("promote to give check", "4k3/1P6/8/8/8/8/K7/8 w - - 0 1", &[(6, 217342)]);
    assert_perft("underpromote to give check", "8/P1k5/K7/8/8/8/8/8 w - - 0 1", &[(6, 92683)]);
    assert_perft("self stalemate", "K1k5/8/P7/8/8/8/8/8 w - - 0 1", &[(6, 2217)]);
    assert_perft("stalemate and checkmate 1", "8/k1P5/8/1K6/8/8/8/8 w - - 0 1", &[(7, 567584)]);
    assert_perft("stalemate and checkmate 2", "8/8/2k5/5q2/5n2/8/5K2/8 b - - 0 1", &[(4, 23527)]);
}
