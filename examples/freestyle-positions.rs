// https://en.wikipedia.org/wiki/Fischer_random_chess_numbering_scheme

use std::collections::BTreeSet as Set;

// fn random(n: usize) {// -> String {
//     let mut position = vec!['-'; 8];
//     let n2 = n / 4;
//     let b1 = n % 4;
//     position[b1] = 'b';
//     let n3 = n2 / 4;
//     let b2 = n2 % 4;
//     position[b2] = 'b';
//     let n4 = n3 / 6;
//     let q = n3 % 6;
//     position[q] = 'q';
//     assert!(n4 < 10);
// }

fn main() {
    // for n in 0..960 {
    //     random(n);
    // }
    let mut positions = Vec::new();
    let all = Set::from_iter(1usize..=8);
    for rook_l in 1usize..=8 {
        for rook_r in rook_l + 2..=8 {
            for king in (rook_l + 1)..rook_r {
                let others = (1usize..rook_l)
                    .chain((rook_l + 1)..king)
                    .chain((king + 1)..rook_r)
                    .chain((rook_r + 1)..=8);
                for bishop_b in others.clone().filter(|i| i & 1 == 1) {
                    for bishop_w in others.clone().filter(|i| i & 1 == 0) {
                        let used = Set::from([rook_l, rook_r, king, bishop_b, bishop_w]);
                        let queens = all.difference(&used);
                        for queen in queens.into_iter() {
                            let mut position = vec!['n'; 8];
                            position[rook_l - 1] = 'r';
                            position[rook_r - 1] = 'r';
                            position[king - 1] = 'k';
                            position[queen - 1] = 'q';
                            position[bishop_w - 1] = 'b';
                            position[bishop_b - 1] = 'b';
                            positions.push(position);
                        }
                    }
                }
            }
        }
    }
    for position in positions.iter() {
        println!("{}", position.iter().collect::<String>());
    }
    assert_eq!(960, positions.len());
}
