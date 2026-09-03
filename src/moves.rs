use core::ops;

use crate::{
    bitboard::{Bitboard, Direction},
    position::{Board, EnPassant, Move, Moves, Piece, Player, Position, Rank, Role, Side, Square},
    variant::Variant,
};

// This would work here too, but it warns about long_running_const_eval
// even if we allow the lint.
// #[allow(long_running_const_eval)]
// static SLIDER_SIGHTS: SliderSights = SliderSights::volker_annuss();
include!("slider_sights.rs");

// move generation
impl<V: Variant> Position<V> {
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
}

impl<V: Variant> Position<V> {
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
        use Role::*;

        let mut moves = Moves::new();

        self.pseudo_pawn_moves(target, &mut moves);
        for role in [Knight, Bishop, Rook, Queen] {
            self.pseudo_role_moves(role, target, &mut moves);
        }

        moves
    }

    fn legal_king_evasion_moves(&self, king: Square, checkers: Bitboard) -> Moves {
        let sliders = checkers.intersection_const(self.board().sliders());
        let mut attacked = Bitboard::EMPTY;
        let mut sliders = sliders;
        while let Some(checker) = sliders.pop_first() {
            // `king_move_is_safe` checks attacks to the destination, but a
            // slider checking the king still controls the ray through the old
            // king square after the king moves away.
            attacked.append_const(
                checker.full_ray(king).difference_const(Bitboard::from_square(checker)),
            );
        }

        let mut moves = Moves::new();
        let target = self.board().player(self.turn()).union_const(attacked);

        self.pseudo_role_moves(Role::King, !target, &mut moves);
        moves.retain(|m| self.king_move_is_safe(*m));

        moves
    }

    pub fn legal_king_moves(&self) -> Moves {
        let mut moves = Moves::new();
        let target = !self.board().player(self.turn());

        self.pseudo_role_moves(Role::King, target, &mut moves);
        moves.retain(|m| self.king_move_is_safe(*m));

        moves
    }

    fn king_move_is_safe(&self, play: Move) -> bool {
        let occupied = self.board().occupied().difference_const(Bitboard::from_square(play.from));
        self.board().attacks_on(play.to, self.turn().other(), occupied).is_empty()
    }

    fn king_square_is_safe(&self, square: Square) -> bool {
        self.board().attacks_on(square, self.turn().other(), self.board().occupied()).is_empty()
    }

    fn pseudo_role_moves(&self, role: Role, target: Bitboard, moves: &mut Moves) {
        let occupied = self.board().occupied();
        let mut pieces =
            self.board().role(role).intersection_const(self.board().player(self.turn()));

        while let Some(from) = pieces.pop_first() {
            let piece = role.of(self.turn());
            let mut targets = from.attacks(piece, occupied).intersection_const(target);
            while let Some(to) = targets.pop_first() {
                moves.push(Move::capture(role, from, to, self.board().role_at(to)));
            }
        }
    }

    fn pseudo_pawn_moves(&self, target: Bitboard, moves: &mut Moves) {
        let occupied = self.board().occupied();
        let them = self.board().player(self.turn().other());
        let pawns = self.board().pawns().intersection_const(self.board().player(self.turn()));
        let empty = !occupied;
        let (push, double_push, left, right, double_rank) = pawn_directions(self.turn());

        let single = pawns.checked_shift(push).intersection_const(empty);
        let double =
            single.intersection_const(double_rank).checked_shift(push).intersection_const(empty);
        let captures_left = pawns.checked_shift(left).intersection_const(them);
        let captures_right = pawns.checked_shift(right).intersection_const(them);

        let mut targets = single.intersection_const(target);
        while let Some(to) = targets.pop_first() {
            let from = to.checked_add_const(push.reverse()).expect("valid pawn source");
            moves.extend(Move::pawn(self.turn(), from, to, None));
        }

        let mut targets = double.intersection_const(target);
        while let Some(to) = targets.pop_first() {
            let from = to.checked_add_const(double_push.reverse()).expect("valid pawn source");
            moves.push(Move::normal(Role::Pawn, from, to));
        }

        let mut targets = captures_left.intersection_const(target);
        while let Some(to) = targets.pop_first() {
            let from = to.checked_add_const(left.reverse()).expect("valid pawn source");
            moves.extend(Move::pawn(self.turn(), from, to, self.board().role_at(to)));
        }

        let mut targets = captures_right.intersection_const(target);
        while let Some(to) = targets.pop_first() {
            let from = to.checked_add_const(right.reverse()).expect("valid pawn source");
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
                .intersection_const(self.board().player(self.turn()))
                .intersection_const(to.pawn_attack_moves(self.turn().other()));

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
            .difference_const(Bitboard::from_square(play.from))
            .difference_const(Bitboard::from_square(captured))
            .union_const(Bitboard::from_square(play.to));

        self.board().attacks_on(king, self.turn().other(), occupied).is_empty()
    }
}

impl<V: Variant> Position<V> {
    pub fn can_castle(&self, side: Side) -> Option<Move> {
        let rook_file = self.castles().get(self.turn(), side)?;
        let king_from = self.board().king_of(self.turn())?;
        let empty_path = self.turn().castle_empty_path(king_from, rook_file);
        if !self.board().occupied().intersection_const(empty_path).is_empty() {
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

impl<V> Position<V> {
    pub const fn effective_en_passant(self) -> Option<EnPassant> {
        let Some(en_passant) = self.en_passant() else {
            return None;
        };

        let to = en_passant.square();

        // If say d6 is the en passant square, check that d5 actually contains a pawn.
        let Some(captured) = to.checked_add_const(self.turn().other().pawn_push()) else {
            return None;
        };
        let Some(piece) = self.board().piece_at(captured) else {
            return None;
        };
        if !piece.eq(self.turn().other().pawn()) {
            return None;
        }

        self.attacked_en_passant(en_passant)
    }

    pub const fn attacked_en_passant(self, en_passant: EnPassant) -> Option<EnPassant> {
        let to = en_passant.square();
        let pawns = self.board().pawns().intersection_const(self.board().player(self.turn()));
        if pawns.intersection_const(to.pawn_attack_moves(self.turn().other())).is_empty() {
            None
        } else {
            Some(en_passant)
        }
    }
}

// king-safety moves
impl Board {
    pub fn king_shields(self, player: Player) -> Bitboard {
        match self.king_of(player) {
            Some(king) => {
                let attacker = self.player(player.other());
                let straight =
                    king.rook_sight(Bitboard::EMPTY).intersection_const(self.rooks_and_queens());
                let diagonal = king
                    .bishop_sight(Bitboard::EMPTY)
                    .intersection_const(self.bishops_and_queens());
                let snipers = straight.union_const(diagonal).intersection_const(attacker);

                let mut shields = Bitboard::EMPTY;
                let mut snipers = snipers;
                while let Some(sniper) = snipers.pop_first() {
                    let blockers = king.between(sniper).intersection_const(self.occupied());
                    if !blockers.more_than_one() {
                        shields.append_const(blockers.intersection_const(self.player(player)));
                    }
                }

                shields
            }
            None => Bitboard::EMPTY,
        }
    }
}

// attack-related moves
impl Board {
    // Nomenclature is a bit confusing and we don't need it so far
    // This is all squares (including both sides) that are in "attack sight" of the piece on the the square
    // pub fn attacks_from(self, square: Square) -> Bitboard {
    //     match self.piece_at(square) {
    //         Some(piece) => square.attacks(piece, self.occupied()),
    //         None => Bitboard::EMPTY,
    //     }
    // }

    /// Pieces of `attacker` on this board that attack `square`.
    ///
    /// `occupied` is the occupancy view used for line-of-sight and for
    /// filtering attackers. This supports king-safety checks after the other
    /// player moves: removed pieces are ignored, and slider rays use the
    /// post-move blockers.
    ///
    /// This is not a full hypothetical-board query for moves by `attacker`,
    /// because piece roles and players still come from `self`.
    pub const fn attacks_on(
        self,
        square: Square,
        attacker: Player,
        occupied: Bitboard,
    ) -> Bitboard {
        // Work backwards from the target square to find possible source squares.
        // Sliders, knights and kings are symmetric enough for this.
        let straight = square.rook_sight(occupied).intersection_const(self.rooks_and_queens());
        let diagonal = square.bishop_sight(occupied).intersection_const(self.bishops_and_queens());
        let knights = square.knight_moves().intersection_const(self.knights());
        let kings = square.king_moves().intersection_const(self.kings());

        // Pawns are directional, so find pawn source squares with the opposite
        // player's pawn attacks, then filter down to `attacker` below.
        let pawns = square.pawn_attack_moves(attacker.other()).intersection_const(self.pawns());

        let attacks = straight
            .union_const(diagonal)
            .union_const(knights)
            .union_const(kings)
            .union_const(pawns);

        // The role filters above include both players' pieces. Intersecting
        // with `occupied` removes pieces that are gone in this occupancy view.
        self.player(attacker).intersection_const(occupied).intersection_const(attacks)
    }
}

impl Square {
    pub const fn attacks(self, piece: Piece, occupied: Bitboard) -> Bitboard {
        match piece.role {
            Role::Pawn => self.pawn_attack_moves(piece.player),
            Role::Knight => self.knight_moves(),
            Role::Bishop => self.bishop_sight(occupied),
            Role::Rook => self.rook_sight(occupied),
            Role::Queen => self.queen_sight(occupied),
            Role::King => self.king_moves(),
        }
    }

    const fn checked_add_const(self, direction: Direction) -> Option<Square> {
        let square = self as i8;
        let target = square + direction as i8;
        // Equivalent to splitting square + step into file + rank,
        // adding coordinates, and then checking if file + rank are in 0..=7
        let file_diff = (target & 0x7) - (square & 0x7);
        if target >= 0 && target < 64 && file_diff >= -2 && file_diff <= 2 {
            Some(Square::panicky_from_index(target as u8))
        } else {
            None
        }
    }

    // Add step vector to square, union into a bitboard
    const fn checked_add_vector_const(self, directions: &[Direction]) -> Bitboard {
        let mut attacks = Bitboard::EMPTY;
        let mut i = 0;
        while i < directions.len() {
            if let Some(target) = self.checked_add_const(directions[i]) {
                attacks.append_const(Bitboard::from_square(target));
            }
            i += 1;
        }
        attacks
    }

    pub const fn king_moves(self) -> Bitboard {
        const KING_ATTACKS: [Direction; 8] = {
            use Direction::*;
            [North, NorthEast, East, SouthEast, South, SouthWest, West, NorthWest]
        };

        self.checked_add_vector_const(&KING_ATTACKS)
    }

    pub const fn knight_moves(self) -> Bitboard {
        const KNIGHT_MOVES: [Direction; 8] = {
            use Direction::*;
            [
                KnightNorthEast,
                KnightEastNorth,
                KnightEastSouth,
                KnightSouthEast,
                KnightSouthWest,
                KnightWestSouth,
                KnightWestNorth,
                KnightNorthWest,
            ]
        };

        self.checked_add_vector_const(&KNIGHT_MOVES)
    }

    pub const fn pawn_attack_moves(self, player: Player) -> Bitboard {
        const WHITE_PAWN_ATTACKS: [Direction; 2] = {
            use Direction::*;
            [NorthWest, NorthEast]
        };
        const BLACK_PAWN_ATTACKS: [Direction; 2] = {
            use Direction::*;
            [SouthWest, SouthEast]
        };

        match player {
            Player::White => self.checked_add_vector_const(&WHITE_PAWN_ATTACKS),
            Player::Black => self.checked_add_vector_const(&BLACK_PAWN_ATTACKS),
        }
    }

    pub const fn bishop_sight(self, occupied: Bitboard) -> Bitboard {
        SLIDER_SIGHTS.bishop_sight(self, occupied)
    }

    pub const fn rook_sight(self, occupied: Bitboard) -> Bitboard {
        SLIDER_SIGHTS.rook_sight(self, occupied)
    }

    pub const fn queen_sight(self, occupied: Bitboard) -> Bitboard {
        self.bishop_sight(occupied).union_const(self.rook_sight(occupied))
    }

    // the full line through the two squares
    pub const fn full_ray(self, other: Square) -> Bitboard {
        Bitboard::FULL_RAYS[self as usize][other as usize]
    }

    // The row-major half-open interval [min(self, other), max(self, other)).
    //
    // For d2, g5, after also removing the first square:
    //
    // 8  . . . . . . . .
    // 7  . . . . . . . .
    // 6  . . . . . . . .
    // 5  x x x x x x . .
    // 4  x x x x x x x x
    // 3  x x x x x x x x
    // 2  . . . . x x x x
    // 1  . . . . . . . .
    //
    //    a b c d e f g h
    const fn index_range(self, other: Square) -> Bitboard {
        Bitboard((!0 << self as u32) ^ (!0 << other as u32))
    }

    // The row-major squares after this one, excluding self..
    // For d2, this includes e2..h8 and excludes a1..d2.
    //
    // For d2:
    //
    // 8  x x x x x x x x
    // 7  x x x x x x x x
    // 6  x x x x x x x x
    // 5  x x x x x x x x
    // 4  x x x x x x x x
    // 3  x x x x x x x x
    // 2  . . . . x x x x
    // 1  . . . . . . . .
    //
    //    a b c d e f g h
    pub const fn index_after(self) -> Bitboard {
        Bitboard(!0 << (self as u32 + 1))
    }

    // The row-major squares before this one, excluding self.
    // For d2, this includes a1..c2 and excludes d2..h8.
    pub const fn index_before(self) -> Bitboard {
        Bitboard((1 << self as u32) - 1)
    }

    pub const fn east(self) -> Bitboard {
        self.index_after().intersection_const(Bitboard::from_rank(self.rank()))
    }

    pub const fn west(self) -> Bitboard {
        self.index_before().intersection_const(Bitboard::from_rank(self.rank()))
    }

    pub const fn between(self, other: Square) -> Bitboard {
        // Intersecting the index range with the geometric ray leaves only the
        // ray segment between the endpoints.
        self.full_ray(other).intersection_const(self.index_range(other)).without_first()
    }

    pub const fn aligned(self, b: Square, c: Square) -> bool {
        self.full_ray(b).contains(c)
    }
}

impl ops::Add<Direction> for Square {
    type Output = Option<Square>;

    fn add(self, direction: Direction) -> Option<Square> {
        self.checked_add_const(direction)
    }
}

impl ops::Add<&[Direction]> for Square {
    type Output = Bitboard;

    fn add(self, directions: &[Direction]) -> Bitboard {
        self.checked_add_vector_const(directions)
    }
}

struct Bishop;
impl Bishop {
    pub const DIRECTIONS: [Direction; 4] = {
        use Direction::*;
        [NorthEast, SouthEast, SouthWest, NorthWest]
    };

    pub const fn projector(square: Square) -> &'static Projector<9> {
        &BISHOP_PROJECTOR[square as usize]
    }

    pub const fn blockers(square: Square) -> Bitboard {
        const BLOCKERS: SliderBlockers = SliderBlockers::new(&Bishop::DIRECTIONS);

        BLOCKERS.get(square)
    }
}

struct Rook;
impl Rook {
    pub const DIRECTIONS: [Direction; 4] = {
        use Direction::*;
        [North, East, South, West]
    };

    pub const fn projector(square: Square) -> &'static Projector<12> {
        &ROOK_PROJECTOR[square as usize]
    }

    pub const fn blockers(square: Square) -> Bitboard {
        const BLOCKERS: SliderBlockers = SliderBlockers::new(&Rook::DIRECTIONS);

        BLOCKERS.get(square)
    }
}

struct Projector<const B: u32> {
    pub factor: u64,
    pub offset: usize,
}

impl<const B: u32> Projector<B> {
    const fn new(factor: u64, offset: usize) -> Self {
        Self { factor, offset }
    }

    // Compress the "occupied squares" bitboard down to 88772 entries
    const fn index(&self, bitboard: Bitboard) -> usize {
        (self.factor.wrapping_mul(bitboard.0) >> (64 - B)) as usize + self.offset
    }
}

pub struct SliderSights([Bitboard; 88772]);

impl SliderSights {
    pub const fn volker_annuss() -> Self {
        let mut this = Self([Bitboard::EMPTY; 88772]);
        let mut index = 0;
        while index < 64 {
            let square = Square::ALL[index];
            this.project_bishop_sights(square);
            this.project_rook_sights(square);
            index += 1;
        }
        this
    }

    pub const fn into_array(self) -> [Bitboard; 88772] {
        self.0
    }

    // All the squares a bishop in square "sees", assuming the given occupied squares.
    // Includes the first occupied squarie blocking further sight
    pub const fn bishop_sight(&self, square: Square, occupied: Bitboard) -> Bitboard {
        let blockers = Bishop::blockers(square);
        let index = Bishop::projector(square).index(occupied.intersection_const(blockers));
        self.0[index]
    }

    pub const fn rook_sight(&self, square: Square, occupied: Bitboard) -> Bitboard {
        let blockers = Rook::blockers(square);
        let index = Rook::projector(square).index(occupied.intersection_const(blockers));
        self.0[index]
    }

    const fn project_bishop_sights(&mut self, square: Square) {
        self.project_slider_sights(
            square,
            &Bishop::DIRECTIONS,
            Bishop::blockers(square),
            Bishop::projector(square),
        );
    }

    const fn project_rook_sights(&mut self, square: Square) {
        self.project_slider_sights(
            square,
            &Rook::DIRECTIONS,
            Rook::blockers(square),
            Rook::projector(square),
        );
    }

    const fn get(&self, index: usize) -> Bitboard {
        self.0[index]
    }

    const fn set(&mut self, index: usize, attack: Bitboard) {
        self.0[index] = attack;
    }

    const fn project_slider_sights<const B: u32>(
        &mut self,
        square: Square,
        directions: &[Direction],
        blockers: Bitboard,
        projector: &Projector<B>,
    ) {
        // It's a numerical trick that
        // s := 0
        // s -> s.wrapping_sub(m) & m
        // cycles through the powerset of m (all 2^popcount(m) subsets).
        //
        // The operation is conjugate to ordinary increment on a dense counter.
        // Let the set bits of m be at positions:
        // p0 < p1 < ... < p(n-1)
        //
        // Define pack(s) as taking a subset s and compressing the mask bits into an n-bit integer:
        // bit(i) of pack(s) = bit(p_i) of s
        //
        // Then for: next(s) = s.wrapping_sub(m) & m
        // We have: pack(next(s)) = pack(s) + 1 mod 2^n
        //
        // Example:
        // m bits: positions 1,2,4
        // s bitboard: 00000 00010 00100 00110 10000 ...
        // pack(s):      000   001   010   011   100 ...
        //
        // So next walks through dense counter values 0, 1, 2, ..., 2^n - 1, just embedded into the sparse positions of m.
        const fn next(s: Bitboard, m: Bitboard) -> Bitboard {
            Bitboard(s.0.wrapping_sub(m.0) & m.0)
        }

        let mut occupied = Bitboard::EMPTY;
        loop {
            let index = projector.index(occupied);
            let sight = Self::sight(square, occupied, directions);
            // sanity check: we are not overwriting an existing attack
            // due to hash / magic index failing by clashing.
            assert!(self.get(index).is_empty() || self.get(index).eq_const(sight));
            self.set(index, sight);
            occupied = next(occupied, blockers);
            if occupied.is_empty() {
                break;
            }
        }
    }

    // This is NOT computed directly for every "slider" attack from a given square.
    // Instead, it's used to precompute the slider attack table.
    const fn sight(square: Square, occupied: Bitboard, directions: &[Direction]) -> Bitboard {
        let mut sight = Bitboard::EMPTY;
        let mut i = 0;
        while i < directions.len() {
            let direction = directions[i];
            let mut square = square;
            while let Some(target) = square.checked_add_const(direction) {
                sight.append_const(Bitboard::from_square(target));
                // hit an occupied square
                if occupied.contains(target) {
                    break;
                }
                square = target;
            }
            i += 1;
        }
        sight
    }
}

#[allow(dead_code)]
struct SliderBlockers([Bitboard; 64]);

#[allow(dead_code)]
impl SliderBlockers {
    const fn new(directions: &[Direction]) -> Self {
        let mut blockers = [Bitboard::EMPTY; 64];
        let mut index = 0;
        while index < 64 {
            blockers[index] = Self::ray_blockers(Square::ALL[index], directions);
            index += 1;
        }
        Self(blockers)
    }

    const fn get(&self, square: Square) -> Bitboard {
        self.0[square as usize]
    }

    const fn ray_blockers(square: Square, directions: &[Direction]) -> Bitboard {
        let mut blockers = Bitboard::EMPTY;
        let mut i = 0;
        while i < directions.len() {
            let direction = directions[i];
            let mut target = square.checked_add_const(direction);
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

impl Player {
    const fn pawn_push(self) -> Direction {
        use {Direction::*, Player::*};

        match self {
            White => North,
            Black => South,
        }
    }
}

const fn pawn_directions(player: Player) -> (Direction, Direction, Direction, Direction, Bitboard) {
    use {Direction::*, Player::*};

    match player {
        White => (North, NorthNorth, NorthWest, NorthEast, Bitboard::from_rank(Rank::Three)),
        Black => (South, SouthSouth, SouthWest, SouthEast, Bitboard::from_rank(Rank::Six)),
    }
}

// Fixed shift white magics found by Volker Annuss.
// From: http://www.talkchess.com/forum/viewtopic.php?p=727500&t=64790

#[rustfmt::skip]
const BISHOP_PROJECTOR: [Projector<9>; 64] = [
    Projector::new(0x007f_bfbf_bfbf_bfff, 5378),
    Projector::new(0x0000_a060_4010_07fc, 4093),
    Projector::new(0x0001_0040_0802_0000, 4314),
    Projector::new(0x0000_8060_0400_0000, 6587),
    Projector::new(0x0000_1004_0000_0000, 6491),
    Projector::new(0x0000_21c1_00b2_0000, 6330),
    Projector::new(0x0000_0400_4100_8000, 5609),
    Projector::new(0x0000_0fb0_203f_ff80, 22236),
    Projector::new(0x0000_0401_0040_1004, 6106),
    Projector::new(0x0000_0200_8020_0802, 5625),
    Projector::new(0x0000_0040_1020_2000, 16785),
    Projector::new(0x0000_0080_6004_0000, 16817),
    Projector::new(0x0000_0044_0200_0000, 6842),
    Projector::new(0x0000_0008_0100_8000, 7003),
    Projector::new(0x0000_07ef_e0bf_ff80, 4197),
    Projector::new(0x0000_0008_2082_0020, 7356),
    Projector::new(0x0000_4000_8080_8080, 4602),
    Projector::new(0x0002_1f01_0040_0808, 4538),
    Projector::new(0x0001_8000_c06f_3fff, 29531),
    Projector::new(0x0000_2582_0080_1000, 45393),
    Projector::new(0x0000_2400_8084_0000, 12420),
    Projector::new(0x0000_1800_0c03_fff8, 15763),
    Projector::new(0x0000_0a58_4020_8020, 5050),
    Projector::new(0x0000_0200_0820_8020, 4346),
    Projector::new(0x0000_8040_0081_0100, 6074),
    Projector::new(0x0001_0119_0080_2008, 7866),
    Projector::new(0x0000_8040_0081_0100, 32139),
    Projector::new(0x0001_0040_3c04_03ff, 57673),
    Projector::new(0x0007_8402_a880_2000, 55365),
    Projector::new(0x0000_1010_0080_4400, 15818),
    Projector::new(0x0000_0808_0010_4100, 5562),
    Projector::new(0x0000_4004_c008_2008, 6390),
    Projector::new(0x0001_0101_2000_8020, 7930),
    Projector::new(0x0000_8080_9a00_4010, 13329),
    Projector::new(0x0007_fefe_0881_0010, 7170),
    Projector::new(0x0003_ff0f_833f_c080, 27267),
    Projector::new(0x007f_e080_1900_3042, 53787),
    Projector::new(0x003f_ffef_ea00_3000, 5097),
    Projector::new(0x0000_1010_1000_2080, 6643),
    Projector::new(0x0000_8020_0508_0804, 6138),
    Projector::new(0x0000_8080_80a8_0040, 7418),
    Projector::new(0x0000_1041_0020_0040, 7898),
    Projector::new(0x0003_ffdf_7f83_3fc0, 42012),
    Projector::new(0x0000_0088_4045_0020, 57350),
    Projector::new(0x0000_7ffc_8018_0030, 22813),
    Projector::new(0x007f_ffdd_8014_0028, 56693),
    Projector::new(0x0002_0080_200a_0004, 5818),
    Projector::new(0x0000_1010_1010_0020, 7098),
    Projector::new(0x0007_ffdf_c180_5000, 4451),
    Projector::new(0x0003_ffef_e0c0_2200, 4709),
    Projector::new(0x0000_0008_2080_6000, 4794),
    Projector::new(0x0000_0000_0840_3000, 13364),
    Projector::new(0x0000_0001_0020_2000, 4570),
    Projector::new(0x0000_0040_4080_2000, 4282),
    Projector::new(0x0004_0100_4010_0400, 14964),
    Projector::new(0x0000_6020_6018_03f4, 4026),
    Projector::new(0x0003_ffdf_dfc2_8048, 4826),
    Projector::new(0x0000_0008_2082_0020, 7354),
    Projector::new(0x0000_0000_0820_8060, 4848),
    Projector::new(0x0000_0000_0080_8020, 15946),
    Projector::new(0x0000_0000_0100_2020, 14932),
    Projector::new(0x0000_0004_0100_2008, 16588),
    Projector::new(0x0000_0040_4040_4040, 6905),
    Projector::new(0x007f_ff9f_df7f_f813, 16076),
];

#[rustfmt::skip]
const ROOK_PROJECTOR: [Projector<12>; 64] = [
    Projector::new(0x0028_0077_ffeb_fffe, 26304),
    Projector::new(0x2004_0102_0109_7fff, 35520),
    Projector::new(0x0010_0200_1005_3fff, 38592),
    Projector::new(0x0040_0400_0800_4002, 8026),
    Projector::new(0x7fd0_0441_ffff_d003, 22196),
    Projector::new(0x4020_0088_87df_fffe, 80870),
    Projector::new(0x0040_0088_8847_ffff, 76747),
    Projector::new(0x0068_00fb_ff75_fffd, 30400),
    Projector::new(0x0000_2801_0113_ffff, 11115),
    Projector::new(0x0020_0402_01fc_ffff, 18205),
    Projector::new(0x007f_e800_42ff_ffe8, 53577),
    Projector::new(0x0000_1800_217f_ffe8, 62724),
    Projector::new(0x0000_1800_073f_ffe8, 34282),
    Projector::new(0x0000_1800_e05f_ffe8, 29196),
    Projector::new(0x0000_1800_602f_ffe8, 23806),
    Projector::new(0x0000_3000_2fff_ffa0, 49481),
    Projector::new(0x0030_0018_010b_ffff, 2410),
    Projector::new(0x0003_000c_0085_fffb, 36498),
    Projector::new(0x0004_0008_0201_0008, 24478),
    Projector::new(0x0004_0020_2002_0004, 10074),
    Projector::new(0x0001_0020_0200_2001, 79315),
    Projector::new(0x0001_0010_0080_1040, 51779),
    Projector::new(0x0000_0040_4000_8001, 13586),
    Projector::new(0x0000_0068_00cd_fff4, 19323),
    Projector::new(0x0040_2000_1008_0010, 70612),
    Projector::new(0x0000_0800_1004_0010, 83652),
    Projector::new(0x0004_0100_0802_0008, 63110),
    Projector::new(0x0000_0400_2020_0200, 34496),
    Projector::new(0x0002_0080_1010_0100, 84966),
    Projector::new(0x0000_0080_2001_0020, 54341),
    Projector::new(0x0000_0080_2020_0040, 60421),
    Projector::new(0x0000_8200_2000_4020, 86402),
    Projector::new(0x00ff_fd18_0030_0030, 50245),
    Projector::new(0x007f_ff7f_bfd4_0020, 76622),
    Projector::new(0x003f_ffbd_0018_0018, 84676),
    Projector::new(0x001f_ffde_8018_0018, 78757),
    Projector::new(0x000f_ffe0_bfe8_0018, 37346),
    Projector::new(0x0001_0000_8020_2001, 370),
    Projector::new(0x0003_fffb_ff98_0180, 42182),
    Projector::new(0x0001_fffd_ff90_00e0, 45385),
    Projector::new(0x00ff_fefe_ebff_d800, 61659),
    Projector::new(0x007f_fff7_ffc0_1400, 12790),
    Projector::new(0x003f_ffbf_e4ff_e800, 16762),
    Projector::new(0x001f_fff0_1fc0_3000, 0),
    Projector::new(0x000f_ffe7_f8bf_e800, 38380),
    Projector::new(0x0007_ffdf_df3f_f808, 11098),
    Projector::new(0x0003_fff8_5fff_a804, 21803),
    Projector::new(0x0001_fffd_75ff_a802, 39189),
    Projector::new(0x00ff_ffd7_ffeb_ffd8, 58628),
    Projector::new(0x007f_ff75_ff7f_bfd8, 44116),
    Projector::new(0x003f_ff86_3fbf_7fd8, 78357),
    Projector::new(0x001f_ffbf_dfd7_ffd8, 44481),
    Projector::new(0x000f_fff8_1028_0028, 64134),
    Projector::new(0x0007_ffd7_f7fe_ffd8, 41759),
    Projector::new(0x0003_fffc_0c48_0048, 1394),
    Projector::new(0x0001_ffff_afd7_ffd8, 40910),
    Projector::new(0x00ff_ffe4_ffdf_a3ba, 66516),
    Projector::new(0x007f_ffef_7ff3_d3da, 3897),
    Projector::new(0x003f_ffbf_dfef_f7fa, 3930),
    Projector::new(0x001f_ffef_f7fb_fc22, 72934),
    Projector::new(0x0000_0204_0800_1001, 72662),
    Projector::new(0x0007_fffe_ffff_77fd, 56325),
    Projector::new(0x0003_ffff_bf7d_feec, 66501),
    Projector::new(0x0001_ffff_9dff_a333, 14826),
];

#[cfg(test)]
mod tests {
    use crate::{
        Player::*,
        position::{Castles, File::*, Position, Side, Square::*},
        variant::Freestyle,
    };

    fn freestyle_position() -> Position<Freestyle> {
        let mut position = Position::empty();
        position.set_piece(C1, White.king());
        position.set_piece(A1, White.rook());
        position.set_piece(H1, White.rook());
        position.set_piece(C8, Black.king());
        position.set_piece(A8, Black.rook());
        position.set_piece(H8, Black.rook());
        let mut parts = position.parts();
        parts.castles = Castles::empty();
        parts.castles.set(White, Side::Queen, A);
        parts.castles.set(White, Side::King, H);
        parts.castles.set(Black, Side::Queen, A);
        parts.castles.set(Black, Side::King, H);
        parts.position().validate().unwrap()
    }

    #[test]
    fn generates_freestyle_castle_moves() {
        let moves = freestyle_position().legal_castle_moves();

        assert!(moves.contains(&crate::Move::castle(White, C1, A)));
        assert!(moves.contains(&crate::Move::castle(White, C1, H)));
    }

    #[test]
    fn blocks_freestyle_castle_move() {
        let mut position = freestyle_position().unvalidated();
        position.set_piece(E1, White.knight());
        let position: Position<Freestyle> = position.validate().unwrap();

        assert!(!position.legal_castle_moves().contains(&crate::Move::castle(White, C1, H)));
    }
}
