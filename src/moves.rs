use core::ops;

use crate::{
    bitboard::Bitboard,
    position::{Player, Square},
};

// This would work here too - but it will warn about long_running_const_eval
// even if we allow the lint.
// #[allow(long_running_const_eval)]
// static SLIDER_ATTACKS: [Bitboard; 88772] = slider_attacks();
include!("slider_attacks.rs");

impl Square {
    const fn checked_add_const(self, step: Step) -> Option<Square> {
        let square = self as i8;
        let target = square + step.0;
        // Equivalent to splitting square + step into file + rank,
        // adding coordinates, and then checking if file + rank are in 0..=7
        let file_diff = (target & 0x7) - (square & 0x7);
        if target >= 0 && target < 64 && file_diff >= -2 && file_diff <= 2 {
            Some(Square::panicky_new(target as u8))
        } else {
            None
        }
    }

    // Add step vector to square, union into a bitboard
    const fn checked_add_vector_const(self, steps: &[Step]) -> Bitboard {
        let mut attacks = Bitboard::EMPTY;
        let mut i = 0;
        while i < steps.len() {
            if let Some(target) = self.checked_add_const(steps[i]) {
                attacks.append_const(Bitboard::from_square(target));
            }
            i += 1;
        }
        attacks
    }

    pub const fn king_attacks(self) -> Bitboard {
        const KING_ATTACKS: [Step; 8] = [
            Step::NORTH_EAST,
            Step::NORTH,
            Step::NORTH_WEST,
            Step::EAST,
            Step::SOUTH_WEST,
            Step::SOUTH,
            Step::SOUTH_EAST,
            Step::WEST,
        ];

        self.checked_add_vector_const(&KING_ATTACKS)
    }

    pub const fn knight_attacks(self) -> Bitboard {
        const KNIGHT_ATTACKS: [Step; 8] = [
            Step::KNIGHT_NORTH_EAST,
            Step::KNIGHT_NORTH_WEST,
            Step::KNIGHT_EAST_NORTH,
            Step::KNIGHT_WEST_NORTH,
            Step::KNIGHT_SOUTH_WEST,
            Step::KNIGHT_SOUTH_EAST,
            Step::KNIGHT_WEST_SOUTH,
            Step::KNIGHT_EAST_SOUTH,
        ];

        self.checked_add_vector_const(&KNIGHT_ATTACKS)
    }

    pub const fn pawn_attacks(self, player: Player) -> Bitboard {
        const WHITE_PAWN_ATTACKS: [Step; 2] = [Step::NORTH_WEST, Step::NORTH_EAST];
        const BLACK_PAWN_ATTACKS: [Step; 2] = [Step::SOUTH_WEST, Step::SOUTH_EAST];

        match player {
            Player::White => self.checked_add_vector_const(&WHITE_PAWN_ATTACKS),
            Player::Black => self.checked_add_vector_const(&BLACK_PAWN_ATTACKS),
        }
    }

    pub const fn bishop_attacks(self, occupied: Bitboard) -> Bitboard {
        let blockers = Bishop::blockers(self);
        let index = Bishop::magic_index(self, occupied.intersection_const(blockers));
        SLIDER_ATTACKS[index]
    }

    pub const fn rook_attacks(self, occupied: Bitboard) -> Bitboard {
        let blockers = Rook::blockers(self);
        let index = Rook::magic_index(self, occupied.intersection_const(blockers));
        SLIDER_ATTACKS[index]
    }

    pub const fn queen_attacks(self, occupied: Bitboard) -> Bitboard {
        self.bishop_attacks(occupied).union_const(self.rook_attacks(occupied))
    }

    // This is NOT computed directly for every "slider" attacks from a given square.
    // Instead, it's precomputed
    const fn ray_attacks(self, occupied: Bitboard, directions: &[Step]) -> Bitboard {
        let mut attacks = Bitboard::EMPTY;
        let mut i = 0;
        while i < directions.len() {
            let direction = directions[i];
            let mut square = self;
            while let Some(target) = square.checked_add_const(direction) {
                attacks.append_const(Bitboard::from_square(target));
                // hit an occupied square
                if occupied.contains(target) {
                    break;
                }
                square = target;
            }
            i += 1;
        }
        attacks
    }

    const fn ray_blockers(self, directions: &[Step]) -> Bitboard {
        let mut blockers = Bitboard::EMPTY;
        let mut i = 0;
        while i < directions.len() {
            let direction = directions[i];
            let mut target = self.checked_add_const(direction);
            while let Some(square) = target {
                target = square.checked_add_const(direction);
                if target.is_some() {
                    blockers.append_const(Bitboard::from_square(square));
                }
            }
            i += 1;
        }
        blockers
    }
}

impl ops::Add<Step> for Square {
    type Output = Option<Square>;

    fn add(self, step: Step) -> Option<Square> {
        self.checked_add_const(step)
    }
}

impl ops::Add<&[Step]> for Square {
    type Output = Bitboard;

    fn add(self, steps: &[Step]) -> Bitboard {
        self.checked_add_vector_const(steps)
    }
}

struct Bishop;
impl Bishop {
    pub const DIRECTIONS: [Step; 4] =
        [Step::NORTH_EAST, Step::NORTH_WEST, Step::SOUTH_WEST, Step::SOUTH_EAST];
    const BLOCKERS: [Bitboard; 64] = slider_blockers(&Self::DIRECTIONS);
    const MAGICS: [Magic; 64] = BISHOP_MAGICS;
    const BITS: u32 = 9;
    pub const fn magic(square: Square) -> &'static Magic {
        &Self::MAGICS[square as usize]
    }
    pub const fn magic_index(square: Square, occupied: Bitboard) -> usize {
        slider_magic_index(Bishop::magic(square), occupied.0, Self::BITS)
    }
    pub const fn blockers(square: Square) -> Bitboard {
        Self::BLOCKERS[square as usize]
    }
}

struct Rook;
impl Rook {
    pub const DIRECTIONS: [Step; 4] = [Step::NORTH, Step::EAST, Step::SOUTH, Step::WEST];
    const BLOCKERS: [Bitboard; 64] = slider_blockers(&Self::DIRECTIONS);
    const MAGICS: [Magic; 64] = ROOK_MAGICS;
    const BITS: u32 = 12;
    pub const fn magic(square: Square) -> &'static Magic {
        &Self::MAGICS[square as usize]
    }
    pub const fn magic_index(square: Square, occupied: Bitboard) -> usize {
        slider_magic_index(Rook::magic(square), occupied.0, Self::BITS)
    }
    pub const fn blockers(square: Square) -> Bitboard {
        Self::BLOCKERS[square as usize]
    }
}

struct Magic {
    pub factor: u64,
    pub offset: usize,
}

impl Magic {
    const fn new(factor: u64, offset: usize) -> Self {
        Self { factor, offset }
    }
}

pub const fn slider_attacks() -> [Bitboard; 88772] {
    let mut table = [Bitboard::EMPTY; 88772];
    let mut index = 0;
    while index < 64 {
        let square = Square::ALL[index];
        bishop_square_attacks(&mut table, square);
        rook_square_attacks(&mut table, square);
        index += 1;
    }
    table
}

const fn slider_blockers(directions: &[Step]) -> [Bitboard; 64] {
    let mut blockers = [Bitboard::EMPTY; 64];
    let mut index = 0;
    while index < 64 {
        blockers[index] = Square::ALL[index].ray_blockers(directions);
        index += 1;
    }
    blockers
}

// Compress the "occupied squares" bitboard down to 88772 entries
const fn slider_magic_index(magic: &Magic, occupied: u64, bits: u32) -> usize {
    (magic.factor.wrapping_mul(occupied) >> (64 - bits)) as usize + magic.offset
}

const fn bishop_square_attacks(table: &mut [Bitboard; 88772], square: Square) {
    slider_square_attacks(
        table,
        square,
        &Bishop::DIRECTIONS,
        Bishop::blockers(square),
        Bishop::magic(square),
        Bishop::BITS,
    );
}

const fn rook_square_attacks(table: &mut [Bitboard; 88772], square: Square) {
    slider_square_attacks(
        table,
        square,
        &Rook::DIRECTIONS,
        Rook::blockers(square),
        Rook::magic(square),
        Rook::BITS,
    );
}

const fn slider_square_attacks(
    table: &mut [Bitboard; 88772],
    square: Square,
    directions: &[Step],
    blockers: Bitboard,
    magic: &Magic,
    bits: u32,
) {
    let blockers = blockers.0;
    let mut occupied = 0;
    loop {
        let attack = square.ray_attacks(Bitboard(occupied), directions);
        let index = slider_magic_index(magic, occupied, bits);
        // sanity check: we are not overwriting an existing attack
        // due to hash / magic index failing by clashing.
        assert!(table[index].0 == 0 || table[index].0 == attack.0);
        table[index] = attack;
        occupied = occupied.wrapping_sub(blockers) & blockers;
        if occupied == 0 {
            break;
        }
    }
}

#[derive(Clone, Copy)]
struct Step(i8);

impl Step {
    const NORTH: Step = Step::new(0, 1);
    const EAST: Step = Step::new(1, 0);
    const SOUTH: Step = Step::new(0, -1);
    const WEST: Step = Step::new(-1, 0);

    const NORTH_EAST: Step = Step::new(1, 1);
    const NORTH_WEST: Step = Step::new(-1, 1);
    const SOUTH_EAST: Step = Step::new(1, -1);
    const SOUTH_WEST: Step = Step::new(-1, -1);

    const KNIGHT_NORTH_EAST: Step = Step::new(1, 2);
    const KNIGHT_NORTH_WEST: Step = Step::new(-1, 2);
    const KNIGHT_EAST_NORTH: Step = Step::new(2, 1);
    const KNIGHT_EAST_SOUTH: Step = Step::new(2, -1);
    const KNIGHT_SOUTH_EAST: Step = Step::new(1, -2);
    const KNIGHT_SOUTH_WEST: Step = Step::new(-1, -2);
    const KNIGHT_WEST_NORTH: Step = Step::new(-2, 1);
    const KNIGHT_WEST_SOUTH: Step = Step::new(-2, -1);

    const fn new(file: i8, rank: i8) -> Step {
        Step(rank * 8 + file)
    }
}

// Fixed shift white magics found by Volker Annuss.
// From: http://www.talkchess.com/forum/viewtopic.php?p=727500&t=64790

#[rustfmt::skip]
const BISHOP_MAGICS: [Magic; 64] = [
    Magic::new(0x007f_bfbf_bfbf_bfff, 5378),
    Magic::new(0x0000_a060_4010_07fc, 4093),
    Magic::new(0x0001_0040_0802_0000, 4314),
    Magic::new(0x0000_8060_0400_0000, 6587),
    Magic::new(0x0000_1004_0000_0000, 6491),
    Magic::new(0x0000_21c1_00b2_0000, 6330),
    Magic::new(0x0000_0400_4100_8000, 5609),
    Magic::new(0x0000_0fb0_203f_ff80, 22236),
    Magic::new(0x0000_0401_0040_1004, 6106),
    Magic::new(0x0000_0200_8020_0802, 5625),
    Magic::new(0x0000_0040_1020_2000, 16785),
    Magic::new(0x0000_0080_6004_0000, 16817),
    Magic::new(0x0000_0044_0200_0000, 6842),
    Magic::new(0x0000_0008_0100_8000, 7003),
    Magic::new(0x0000_07ef_e0bf_ff80, 4197),
    Magic::new(0x0000_0008_2082_0020, 7356),
    Magic::new(0x0000_4000_8080_8080, 4602),
    Magic::new(0x0002_1f01_0040_0808, 4538),
    Magic::new(0x0001_8000_c06f_3fff, 29531),
    Magic::new(0x0000_2582_0080_1000, 45393),
    Magic::new(0x0000_2400_8084_0000, 12420),
    Magic::new(0x0000_1800_0c03_fff8, 15763),
    Magic::new(0x0000_0a58_4020_8020, 5050),
    Magic::new(0x0000_0200_0820_8020, 4346),
    Magic::new(0x0000_8040_0081_0100, 6074),
    Magic::new(0x0001_0119_0080_2008, 7866),
    Magic::new(0x0000_8040_0081_0100, 32139),
    Magic::new(0x0001_0040_3c04_03ff, 57673),
    Magic::new(0x0007_8402_a880_2000, 55365),
    Magic::new(0x0000_1010_0080_4400, 15818),
    Magic::new(0x0000_0808_0010_4100, 5562),
    Magic::new(0x0000_4004_c008_2008, 6390),
    Magic::new(0x0001_0101_2000_8020, 7930),
    Magic::new(0x0000_8080_9a00_4010, 13329),
    Magic::new(0x0007_fefe_0881_0010, 7170),
    Magic::new(0x0003_ff0f_833f_c080, 27267),
    Magic::new(0x007f_e080_1900_3042, 53787),
    Magic::new(0x003f_ffef_ea00_3000, 5097),
    Magic::new(0x0000_1010_1000_2080, 6643),
    Magic::new(0x0000_8020_0508_0804, 6138),
    Magic::new(0x0000_8080_80a8_0040, 7418),
    Magic::new(0x0000_1041_0020_0040, 7898),
    Magic::new(0x0003_ffdf_7f83_3fc0, 42012),
    Magic::new(0x0000_0088_4045_0020, 57350),
    Magic::new(0x0000_7ffc_8018_0030, 22813),
    Magic::new(0x007f_ffdd_8014_0028, 56693),
    Magic::new(0x0002_0080_200a_0004, 5818),
    Magic::new(0x0000_1010_1010_0020, 7098),
    Magic::new(0x0007_ffdf_c180_5000, 4451),
    Magic::new(0x0003_ffef_e0c0_2200, 4709),
    Magic::new(0x0000_0008_2080_6000, 4794),
    Magic::new(0x0000_0000_0840_3000, 13364),
    Magic::new(0x0000_0001_0020_2000, 4570),
    Magic::new(0x0000_0040_4080_2000, 4282),
    Magic::new(0x0004_0100_4010_0400, 14964),
    Magic::new(0x0000_6020_6018_03f4, 4026),
    Magic::new(0x0003_ffdf_dfc2_8048, 4826),
    Magic::new(0x0000_0008_2082_0020, 7354),
    Magic::new(0x0000_0000_0820_8060, 4848),
    Magic::new(0x0000_0000_0080_8020, 15946),
    Magic::new(0x0000_0000_0100_2020, 14932),
    Magic::new(0x0000_0004_0100_2008, 16588),
    Magic::new(0x0000_0040_4040_4040, 6905),
    Magic::new(0x007f_ff9f_df7f_f813, 16076),
];

#[rustfmt::skip]
const ROOK_MAGICS: [Magic; 64] = [
    Magic::new(0x0028_0077_ffeb_fffe, 26304),
    Magic::new(0x2004_0102_0109_7fff, 35520),
    Magic::new(0x0010_0200_1005_3fff, 38592),
    Magic::new(0x0040_0400_0800_4002, 8026),
    Magic::new(0x7fd0_0441_ffff_d003, 22196),
    Magic::new(0x4020_0088_87df_fffe, 80870),
    Magic::new(0x0040_0088_8847_ffff, 76747),
    Magic::new(0x0068_00fb_ff75_fffd, 30400),
    Magic::new(0x0000_2801_0113_ffff, 11115),
    Magic::new(0x0020_0402_01fc_ffff, 18205),
    Magic::new(0x007f_e800_42ff_ffe8, 53577),
    Magic::new(0x0000_1800_217f_ffe8, 62724),
    Magic::new(0x0000_1800_073f_ffe8, 34282),
    Magic::new(0x0000_1800_e05f_ffe8, 29196),
    Magic::new(0x0000_1800_602f_ffe8, 23806),
    Magic::new(0x0000_3000_2fff_ffa0, 49481),
    Magic::new(0x0030_0018_010b_ffff, 2410),
    Magic::new(0x0003_000c_0085_fffb, 36498),
    Magic::new(0x0004_0008_0201_0008, 24478),
    Magic::new(0x0004_0020_2002_0004, 10074),
    Magic::new(0x0001_0020_0200_2001, 79315),
    Magic::new(0x0001_0010_0080_1040, 51779),
    Magic::new(0x0000_0040_4000_8001, 13586),
    Magic::new(0x0000_0068_00cd_fff4, 19323),
    Magic::new(0x0040_2000_1008_0010, 70612),
    Magic::new(0x0000_0800_1004_0010, 83652),
    Magic::new(0x0004_0100_0802_0008, 63110),
    Magic::new(0x0000_0400_2020_0200, 34496),
    Magic::new(0x0002_0080_1010_0100, 84966),
    Magic::new(0x0000_0080_2001_0020, 54341),
    Magic::new(0x0000_0080_2020_0040, 60421),
    Magic::new(0x0000_8200_2000_4020, 86402),
    Magic::new(0x00ff_fd18_0030_0030, 50245),
    Magic::new(0x007f_ff7f_bfd4_0020, 76622),
    Magic::new(0x003f_ffbd_0018_0018, 84676),
    Magic::new(0x001f_ffde_8018_0018, 78757),
    Magic::new(0x000f_ffe0_bfe8_0018, 37346),
    Magic::new(0x0001_0000_8020_2001, 370),
    Magic::new(0x0003_fffb_ff98_0180, 42182),
    Magic::new(0x0001_fffd_ff90_00e0, 45385),
    Magic::new(0x00ff_fefe_ebff_d800, 61659),
    Magic::new(0x007f_fff7_ffc0_1400, 12790),
    Magic::new(0x003f_ffbf_e4ff_e800, 16762),
    Magic::new(0x001f_fff0_1fc0_3000, 0),
    Magic::new(0x000f_ffe7_f8bf_e800, 38380),
    Magic::new(0x0007_ffdf_df3f_f808, 11098),
    Magic::new(0x0003_fff8_5fff_a804, 21803),
    Magic::new(0x0001_fffd_75ff_a802, 39189),
    Magic::new(0x00ff_ffd7_ffeb_ffd8, 58628),
    Magic::new(0x007f_ff75_ff7f_bfd8, 44116),
    Magic::new(0x003f_ff86_3fbf_7fd8, 78357),
    Magic::new(0x001f_ffbf_dfd7_ffd8, 44481),
    Magic::new(0x000f_fff8_1028_0028, 64134),
    Magic::new(0x0007_ffd7_f7fe_ffd8, 41759),
    Magic::new(0x0003_fffc_0c48_0048, 1394),
    Magic::new(0x0001_ffff_afd7_ffd8, 40910),
    Magic::new(0x00ff_ffe4_ffdf_a3ba, 66516),
    Magic::new(0x007f_ffef_7ff3_d3da, 3897),
    Magic::new(0x003f_ffbf_dfef_f7fa, 3930),
    Magic::new(0x001f_ffef_f7fb_fc22, 72934),
    Magic::new(0x0000_0204_0800_1001, 72662),
    Magic::new(0x0007_fffe_ffff_77fd, 56325),
    Magic::new(0x0003_ffff_bf7d_feec, 66501),
    Magic::new(0x0001_ffff_9dff_a333, 14826),
];
