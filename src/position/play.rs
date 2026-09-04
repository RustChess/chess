use core::fmt;

use crate::{Player, Role, Square, square::File};

use super::Side;

use File::*;
use Role::*;

#[cfg(test)]
use Player::*;

// Since `move` is a keyword, can use `play: Move` as a synonym.
// At least this is the most useful suggestion by Google AI Overview ;)
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
/// Move that a player can make.
pub struct Move {
    pub kind: Kind,
    /// redundant when given the board
    pub role: Role,
    pub from: Square,
    pub to: Square,
    /// redundant when given the board
    pub capture: Option<Role>,
}

/// What kind of move is it?
#[repr(u8)]
#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub enum Kind {
    #[default]
    Normal,
    Special(Special),
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Special {
    EnPassant,
    Castle(File),
    Promote(Role),
}

impl Kind {
    #[inline]
    pub const fn normal() -> Self {
        Kind::Normal
    }

    #[inline]
    pub const fn special(special: Special) -> Self {
        Kind::Special(special)
    }

    #[inline]
    pub const fn en_passant() -> Self {
        Kind::Special(Special::EnPassant)
    }

    #[inline]
    pub const fn castle(file: File) -> Self {
        Kind::Special(Special::Castle(file))
    }

    #[inline]
    pub const fn promote(role: Role) -> Self {
        Kind::Special(Special::Promote(role))
    }

    #[inline]
    pub const fn is_normal(self) -> bool {
        matches!(self, Kind::Normal)
    }

    #[inline]
    pub const fn is_special(self) -> bool {
        matches!(self, Kind::Normal)
    }

    #[inline]
    pub const fn specials(self) -> Option<Special> {
        if let Kind::Special(special) = self { Some(special) } else { None }
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        if let Kind::Special(special) = self { special.is_en_passant() } else { false }
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        if let Kind::Special(special) = self { special.is_castle() } else { false }
    }

    #[inline]
    pub const fn castle_rook_file(self) -> Option<File> {
        if let Kind::Special(special) = self { special.castle_rook_file() } else { None }
    }

    #[inline]
    pub const fn is_promote(self) -> bool {
        if let Kind::Special(special) = self { special.is_promote() } else { false }
    }

    #[inline]
    pub const fn promotes(self) -> Option<Role> {
        if let Kind::Special(special) = self { special.promotes() } else { None }
    }

    #[inline]
    pub const fn is_promote_role(self, role: Role) -> bool {
        if let Kind::Special(special) = self { special.is_promote_role(role) } else { false }
    }
}

// Construct a normal move, or use the special constructors castle, en_passant, or promote.
//
// If a piece is captured, optionally set its role.

/// Constructors
impl Move {
    #[inline]
    pub const fn normal(role: Role, from: Square, to: Square) -> Move {
        Move::capture(role, from, to, None)
    }

    #[inline]
    pub const fn capture(role: Role, from: Square, to: Square, capture: Option<Role>) -> Move {
        Move { kind: Kind::Normal, role, from, to, capture }
    }

    #[inline]
    pub const fn chess_castle(player: Player, side: Side) -> Move {
        Move::castle(player, Square::new(E, player.backrank()), side.chess_rook())
    }

    #[inline]
    pub const fn castle(player: Player, from: Square, file: File) -> Move {
        let side = Side::of_rook(from, file);
        Move {
            kind: Kind::Special(Special::Castle(file)),
            role: King,
            from,
            to: player.castle_king_to(side),
            capture: None,
        }
    }

    #[inline]
    pub const fn en_passant(from: Square, to: Square) -> Move {
        Move { role: Pawn, from, to, capture: None, kind: Kind::Special(Special::EnPassant) }
    }

    #[inline]
    pub const fn promote(from: Square, to: Square, role: Role) -> Move {
        Move::promote_capture(from, to, role, None)
    }

    #[inline]
    pub const fn promote_capture(
        from: Square,
        to: Square,
        role: Role,
        capture: Option<Role>,
    ) -> Move {
        Move { role: Pawn, from, to, capture, kind: Kind::Special(Special::Promote(role)) }
    }

    pub fn pawn(player: Player, from: Square, to: Square, capture: Option<Role>) -> Vec<Move> {
        let mut moves = Vec::new();

        if to.rank() as u8 == player.promotion_rank() as u8 {
            for role in [Queen, Rook, Bishop, Knight] {
                moves.push(Move::promote_capture(from, to, role, capture));
            }
        } else {
            moves.push(Move::capture(Pawn, from, to, capture));
        }

        moves
    }
}

/// Builders
impl Move {
    #[inline]
    pub const fn capturing(mut self, role: Role) -> Move {
        self.capture = Some(role);
        self
    }
}

/// Accessors inherited from [`Kind`]
impl Move {
    #[inline]
    pub const fn is_normal(self) -> bool {
        self.kind.is_normal()
    }

    #[inline]
    pub const fn is_special(self) -> bool {
        self.kind.is_special()
    }

    #[inline]
    pub const fn specials(self) -> Option<Special> {
        self.kind.specials()
    }
}

/// Accessors inherited from [`Special`]
impl Move {
    #[inline]
    pub const fn is_en_passant(self) -> bool {
        self.kind.is_en_passant()
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        self.kind.is_castle()
    }

    #[inline]
    pub const fn castle_rook_file(self) -> Option<File> {
        self.kind.castle_rook_file()
    }

    #[inline]
    pub const fn is_castle_side(self, side: Side) -> bool {
        match self.castle_side() {
            Some(this) => this.eq(side),
            None => false,
        }
    }

    #[inline]
    pub const fn castle_side(self) -> Option<Side> {
        match self.castle_rook_file() {
            Some(file) => Some(Side::of_rook(self.from, file)),
            None => None,
        }
    }

    #[inline]
    pub const fn is_promote(self) -> bool {
        self.kind.is_promote()
    }

    #[inline]
    pub const fn promotes(self) -> Option<Role> {
        self.kind.promotes()
    }

    #[inline]
    pub const fn is_promote_role(self, role: Role) -> bool {
        self.kind.is_promote_role(role)
    }
}

/// Accessors inherited from `capture`
impl Move {
    #[inline]
    pub const fn is_capture(self) -> bool {
        self.capture.is_some()
    }

    #[inline]
    pub const fn captures(self) -> Option<Role> {
        self.capture
    }

    #[inline]
    pub const fn is_capture_role(self, role: Role) -> bool {
        if let Some(this) = self.capture { this as u8 == role as u8 } else { false }
    }
}

/// Compact encoding
impl Move {
    #[inline]
    pub const fn code(self) -> u16 {
        (self.to as u16)
            | ((self.from as u16) << 6)
            | (self.promotion_code() << 12)
            | (self.kind_code() << 14)
    }

    #[inline]
    const fn promotion_code(self) -> u16 {
        match self.promotes() {
            Some(Knight) => 0,
            Some(Bishop) => 1,
            Some(Rook) => 2,
            Some(Queen) => 3,
            _ => 0,
        }
    }

    #[inline]
    const fn kind_code(self) -> u16 {
        match self.specials() {
            Some(Special::Promote(_)) => 1,
            Some(Special::EnPassant) => 2,
            Some(Special::Castle(_)) => 3,
            None => 0,
        }
    }

    fn long_algebraic(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use algebraic::*;
        use fmt::Write as _;

        let Move { role, from, to, capture, kind } = *self;

        if let Some(side) = self.castle_side() {
            f.write_str(if side == Side::King { SHORT_CASTLE } else { LONG_CASTLE })
        } else {
            if role != Pawn {
                f.write_char(role.upper())?;
            }
            let does = if capture.is_some() { CAPTURE } else { MOVE };
            write!(f, "{}{}{}", from, does, to)?;

            if let Some(promoted) = kind.promotes() {
                write!(f, "={}", promoted.upper())?;
            }
            Ok(())
        }
    }
}

/// Uses long algebraic notation.
impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.long_algebraic(f)
    }
}

impl Special {
    #[inline]
    pub const fn en_passant() -> Self {
        Special::EnPassant
    }

    #[inline]
    pub const fn castle(file: File) -> Self {
        Special::Castle(file)
    }

    #[inline]
    pub const fn promote(role: Role) -> Self {
        Special::Promote(role)
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        matches!(self, Special::EnPassant)
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        matches!(self, Special::Castle(_))
    }

    #[inline]
    pub const fn castle_rook_file(self) -> Option<File> {
        if let Special::Castle(file) = self { Some(file) } else { None }
    }

    #[inline]
    pub const fn is_promote(self) -> bool {
        matches!(self, Special::Promote(_))
    }

    #[inline]
    pub const fn promotes(self) -> Option<Role> {
        if let Special::Promote(role) = self { Some(role) } else { None }
    }

    #[inline]
    pub const fn is_promote_role(self, role: Role) -> bool {
        if let Special::Promote(this) = self { this as u8 == role as u8 } else { false }
    }
}

pub mod algebraic {
    // Note: This is FIDE notation, even though "O-O" and "O-O-O" is prettier
    pub mod fide {
        pub const SHORT_CASTLE: &str = "0-0";
        pub const LONG_CASTLE: &str = "0-0-0";
    }
    // Note: PGN uses vowel O instead of number 0
    pub mod pgn {
        pub const SHORT_CASTLE: &str = "O-O";
        pub const LONG_CASTLE: &str = "O-O-O";
    }

    pub const SHORT_CASTLE: &str = pgn::SHORT_CASTLE;
    pub const LONG_CASTLE: &str = pgn::LONG_CASTLE;
    pub const MOVE: char = '-';
    pub const CAPTURE: char = 'x';
}

#[test]
fn display_move() {
    use Square::*;

    let mut play = Move::promote(A2, H7, Queen);

    assert_eq!(play.to_string(), "a2-h7=Q");
    assert_eq!(play.uci_chess().to_string(), "a2h7q");

    play.role = King;
    assert_eq!(play.to_string(), "Ka2-h7=Q");
    assert_eq!(play.uci_chess().to_string(), "a2h7q");

    play = play.capturing(Bishop);
    assert_eq!(play.to_string(), "Ka2xh7=Q");
    assert_eq!(play.uci_chess().to_string(), "a2h7q");

    let play = Move::chess_castle(White, Side::King);
    assert_eq!(play.to_string(), algebraic::SHORT_CASTLE);
    assert_eq!(play.uci_chess().to_string(), "e1g1");

    let play = Move::chess_castle(Black, Side::Queen);
    assert_eq!(play.to_string(), algebraic::LONG_CASTLE);
    assert_eq!(play.uci_chess().to_string(), "e8c8");

    let play = Move::castle(White, G1, F);
    assert_eq!(play.to_string(), algebraic::LONG_CASTLE);
    assert_eq!(play.uci_chess().to_string(), "g1c1");
    assert_eq!(play.uci_freestyle().to_string(), "g1f1");
}
