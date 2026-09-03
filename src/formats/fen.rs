use core::num::NonZeroU32;

use crate::{
    bitboard::Bitboard,
    finite::Empty as _,
    position::{
        Board, Castles, Chess, EnPassant, File, Freestyle, Parts, Piece, Placement, Player,
        PlayerTable, Position, Rank, Side, Square, VariantEnum,
    },
    variant::{Unvalidated, Variant},
};

use super::{StrInput as Input, prelude::*};

// There's a choice to be made whether to require in-between whitespace or not,
// we accept "compact" FEN without it. The "board" parser finishes once the
// 64 squares are filled, so it won't "swallow" the turn parser's input.

// pub struct Fen(String);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid FEN: {0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

fn backtrack() -> ErrMode<ContextError> {
    ErrMode::Backtrack(ContextError::new())
}

// Lenient - missing suffix fields are filled with default values.
// Missing castling rights are treated like "-", not inferred as KQkq.
pub fn parse_position(input: &mut Input<'_>) -> ModalResult<Position<Unvalidated>> {
    backtrack_err(preceded(multispace0, position)).parse_next(input)
}

fn position(input: &mut Input<'_>) -> ModalResult<Position<Unvalidated>> {
    let board = board.parse_next(input)?;
    let fields = fields.parse_next(input)?;
    let castles = resolve_castles(board, fields.castle_rights).ok_or_else(backtrack)?;
    Ok(Parts {
        board,
        turn: fields.turn,
        castles,
        en_passant: fields.en_passant,
        reversible: fields.reversible,
        round: fields.round,
    }
    .position())
}

impl<V: Variant> Position<V> {
    pub fn from_fen(fen: &str) -> Result<Self> {
        let position = Unvalidated::from_fen(fen)?;
        V::validate(position).map_err(|_| Error::Invalid(fen.to_string()))
    }
}

impl Chess {
    pub fn from_fen(fen: &str) -> Result<Position<Self>> {
        Position::from_fen(fen)
    }
}

impl Freestyle {
    pub fn from_fen(fen: &str) -> Result<Position<Self>> {
        Position::from_fen(fen)
    }
}

impl Unvalidated {
    // implementing on Unvalidated instead of Position<Unvalidated> on purpose,
    // to avoid "duplicate from_fen" in natural call sites.
    pub fn from_fen(fen: &str) -> Result<Position<Self>> {
        parse_position.parse(fen).map_err(|_| Error::Invalid(fen.to_string()))
    }
}

impl<V> Position<V> {
    pub fn apparent_fen(&self) -> String {
        format!("{} {}", self.board().fen(), self.turn().fen(),)
    }
}

impl<V: Variant> Position<V> {
    pub fn fen(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.apparent_fen(),
            self.castles().fen::<V>(),
            en_passant_square(self.en_passant()),
            self.reversible(),
            self.round()
        )
    }

    pub fn transposition_fen(&self) -> String {
        format!(
            "{} {} {}",
            self.apparent_fen(),
            self.castles().fen::<V>(),
            en_passant_square(self.effective_en_passant()),
        )
    }
}

impl Board {
    pub fn fen(self) -> String {
        let mut fen = String::new();

        for rank in Rank::iter_rev() {
            if rank != Rank::Eight {
                fen.push('/');
            }

            let mut empty = 0;
            for file in File::iter() {
                let square = Square::new(file, rank);
                if let Some(piece) = self.piece_at(square) {
                    if empty > 0 {
                        fen.push(char::from_digit(empty, 10).unwrap());
                        empty = 0;
                    }
                    fen.push(piece.char());
                } else {
                    empty += 1;
                }
            }
            if empty > 0 {
                fen.push(char::from_digit(empty, 10).unwrap());
            }
        }

        fen
    }
}

impl Player {
    pub fn fen(self) -> char {
        match self {
            Player::Black => 'b',
            Player::White => 'w',
        }
    }
}

impl Castles {
    pub fn chess_fen(self) -> String {
        use Player::*;
        use Side::*;

        let mut fen = String::new();
        if self.has(White, King) {
            fen.push('K');
        }
        if self.has(White, Queen) {
            fen.push('Q');
        }
        if self.has(Black, King) {
            fen.push('k');
        }
        if self.has(Black, Queen) {
            fen.push('q');
        }
        if fen.is_empty() {
            fen.push('-');
        }
        fen
    }

    pub fn shredder_fen(self) -> String {
        use Player::*;
        use Side::*;

        let mut fen = String::new();
        for (player, side) in [(White, King), (White, Queen), (Black, King), (Black, Queen)] {
            if let Some(file) = self.get(player, side) {
                let letter = if player.is_white() { file.upper() } else { file.lower() };
                fen.push(letter);
            }
        }
        if fen.is_empty() {
            fen.push('-');
        }
        fen
    }

    pub fn fen<V: Variant>(self) -> String {
        match V::VARIANT {
            VariantEnum::Chess => self.chess_fen(),
            // Debatable for Unvalidated, but chess_fen can lose information,
            // so if you want chess_fen, validate the file first.
            VariantEnum::Freestyle | VariantEnum::Unvalidated => self.shredder_fen(),
        }
    }
}

pub fn board(input: &mut Input<'_>) -> ModalResult<Board> {
    let mut placement = Placement::default();
    for rank in Rank::iter_rev() {
        if rank != Rank::Eight {
            '/'.parse_next(input)?;
        }

        placement |= board_row(rank).parse_next(input)?;
    }
    Ok(placement.board())
}

fn board_row(rank: Rank) -> impl FnMut(&mut Input<'_>) -> ModalResult<Placement> {
    move |input| {
        let mut row = Placement::default();
        let mut files = File::cursor();
        loop {
            if files.done() {
                return Ok(row);
            }

            let char = board_fen_char.parse_next(input)?;
            match char {
                i @ '1'..='8' => {
                    if !files.skip(i as u8 - b'0') {
                        return Err(backtrack());
                    }
                }
                piece => {
                    let Some(file) = files.next() else {
                        return Err(backtrack());
                    };
                    let square = Bitboard::from(Square::new(file, rank));
                    let piece = Piece::panicky_from_char(piece);
                    row.players[piece.player] |= square;
                    row.roles[piece.role] |= square;
                }
            }
        }
    }
}

struct Fields {
    turn: Player,
    castle_rights: CastleRights,
    en_passant: Option<EnPassant>,
    reversible: u32,
    round: NonZeroU32,
}

fn fields(input: &mut Input<'_>) -> ModalResult<Fields> {
    // Missing suffix fields are defaulted. Once a field separator is present,
    // cut_err prevents malformed field content from backtracking into "missing".
    let counters = opt_field((reversible, opt_field(round)));
    let suffix = opt_field((turn, opt_field((castle_rights, opt_field((en_passant, counters))))))
        .parse_next(input)?;

    let mut fields = Fields {
        turn: Player::White,
        castle_rights: CastleRights::empty(),
        en_passant: None,
        reversible: 0,
        round: NonZeroU32::MIN,
    };

    let Some((turn, suffix)) = suffix else {
        return Ok(fields);
    };
    fields.turn = turn;

    let Some((castle_rights, suffix)) = suffix else {
        return Ok(fields);
    };
    fields.castle_rights = castle_rights;

    let Some((en_passant, suffix)) = suffix else {
        return Ok(fields);
    };
    fields.en_passant = en_passant;

    let Some((reversible, round)) = suffix else {
        return Ok(fields);
    };
    fields.reversible = reversible;

    if let Some(round) = round {
        fields.round = round;
    }

    Ok(fields)
}

fn en_passant_square(en_passant: Option<EnPassant>) -> String {
    en_passant.map_or_else(|| "-".to_string(), |square| Square::from(square).to_string())
}

fn board_fen_char(input: &mut Input<'_>) -> ModalResult<char> {
    one_of(|c| "12345678pnbrkqPNBRKQ".contains(c)).parse_next(input)
}

fn turn(input: &mut Input<'_>) -> ModalResult<Player> {
    one_of(|c| "bw".contains(c))
        .map(|c| match c {
            'b' => Player::Black,
            'w' => Player::White,
            _ => unreachable!(),
        })
        .parse_next(input)
}

type CastleRights = PlayerTable<Vec<CastleRight>>;

#[derive(Clone, Copy, Eq, PartialEq)]
enum CastleRight {
    File(File),
    Side(Side),
}

impl From<File> for CastleRight {
    fn from(file: File) -> Self {
        Self::File(file)
    }
}

impl From<Side> for CastleRight {
    fn from(side: Side) -> Self {
        Self::Side(side)
    }
}

fn resolve_castles(board: Board, rights: CastleRights) -> Option<Castles> {
    let mut castles = Castles::empty();

    for player in Player::iter() {
        let rights = rights.get_ref(player);
        if rights.is_empty() {
            continue;
        }

        let king = board.king_of(player)?;
        for &right in rights.iter() {
            // Determine the side and file of the castle right.
            // The side is determined in terms of the king's position.
            // For the file:
            //   - For Shredder FEN, the file is directly named
            //   - For standard chess FEN, K/Q/k/q would directly answer both,
            //     but we also want to support X-FEN, where we need to determine
            //     the file from the backrank.
            let (side, file) = match right {
                CastleRight::File(file) => (Side::of_rook(king, file), file),
                CastleRight::Side(side) => (side, x_fen_rook(board, player, king, side)?),
            };
            // Castle rights must be on different sides of the king
            if castles.has(player, side) {
                return None;
            }
            castles.set(player, side, file);
        }
    }

    Some(castles)
}

// Resolve an X-FEN K/Q/k/q right to a rook file.
// The right names the outermost same-colored rook on that side of the king.
fn x_fen_rook(board: Board, player: Player, king: Square, side: Side) -> Option<File> {
    let backrank_rooks = board
        .rooks()
        .intersection(board.player(player))
        .intersection(Bitboard::from_rank(player.backrank()));
    match side {
        Side::King => backrank_rooks.intersection(king.east()).last(),
        Side::Queen => backrank_rooks.intersection(king.west()).first(),
    }
    .map(Square::file)
}

fn castle_rights(input: &mut Input<'_>) -> ModalResult<CastleRights> {
    alt(('-'.value(CastleRights::empty()), some_castles)).parse_next(input)
}

fn some_castles(input: &mut Input<'_>) -> ModalResult<CastleRights> {
    use Player::*;

    let mut rights = PlayerTable::default();
    rights[White] = opt(player_castles(White)).parse_next(input)?.unwrap_or_default();
    rights[Black] = opt(player_castles(Black)).parse_next(input)?.unwrap_or_default();

    if rights.is_empty() {
        return Err(backtrack());
    }

    Ok(rights)
}

fn player_castles<'i>(
    player: Player,
) -> impl FnMut(&mut Input<'i>) -> ModalResult<Vec<CastleRight>> {
    move |input: &mut Input<'i>| {
        let mut castle_letter = castle_letter(player);
        let first = castle_letter.parse_next(input)?;
        let mut rights = vec![first];

        if let Some(second) = opt(&mut castle_letter).parse_next(input)? {
            if first == second {
                return Err(backtrack());
            }
            rights.push(second);
        }

        Ok(rights)
    }
}

fn castle_letter<'i>(player: Player) -> impl FnMut(&mut Input<'i>) -> ModalResult<CastleRight> {
    move |input: &mut Input<'i>| {
        let letters = if player.is_black() { "abcdefghkq" } else { "ABCDEFGHKQ" };
        one_of(|c| letters.contains(c))
            .map(|letter: char| match letter.to_ascii_lowercase() {
                'k' => Side::King.into(),
                'q' => Side::Queen.into(),
                'a'..='h' => File::panicky_from_char(letter).into(),
                _ => unreachable!(),
            })
            .parse_next(input)
    }
}

fn file(input: &mut Input<'_>) -> ModalResult<File> {
    one_of(|c| "abcdefgh".contains(c)).map(File::panicky_from_char).parse_next(input)
}

fn en_passant(input: &mut Input<'_>) -> ModalResult<Option<EnPassant>> {
    alt((
        '-'.value(None),
        terminated(file, '3').map(|file| Some(Square::new(file, Rank::Three).try_into().unwrap())),
        terminated(file, '6').map(|file| Some(Square::new(file, Rank::Six).try_into().unwrap())),
    ))
    .parse_next(input)
}

fn reversible(input: &mut Input<'_>) -> ModalResult<u32> {
    dec_uint.parse_next(input)
}

fn round(input: &mut Input<'_>) -> ModalResult<NonZeroU32> {
    dec_uint.verify_map(NonZeroU32::new).parse_next(input)
}

fn opt_field<'i, O>(
    parser: impl Parser<Input<'i>, O, ErrMode<ContextError>>,
) -> impl Parser<Input<'i>, Option<O>, ErrMode<ContextError>> {
    opt(preceded(space1, cut_err(parser)))
}

#[test]
fn board_fen_example() {
    use File::*;
    use Player::*;
    use Rank::*;
    use Side::*;

    // println!("{:?}", board_fen.parse("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR").unwrap());
    // println!(
    //     "{:?}",
    //     board_fen.parse_next(&mut "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNRxxx").unwrap()
    // );

    let fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3 1 3";
    let position = parse_position.parse(fen).unwrap();
    assert_eq!(position.turn(), Black);
    assert!(position.castles().has(Black, King));
    assert!(position.castles().has(Black, Queen));
    assert!(position.castles().has(White, King));
    assert!(position.castles().has(White, Queen));
    assert_eq!(position.en_passant().map(Into::into), Some(Square::new(E, Three)));
    assert_eq!(position.reversible(), 1);
    assert_eq!(u32::from(position.round()), 3);
    assert_eq!(
        position.validate::<Chess>().unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq - 1 3"
    );
    assert_eq!(
        Chess::from_fen(fen).unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq - 1 3"
    );

    let partial_fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq e3";
    let position = parse_position.parse(partial_fen).unwrap();
    assert_eq!(position.turn(), Black);
    assert!(position.castles().has(Black, King));
    assert!(position.castles().has(Black, Queen));
    assert!(position.castles().has(White, King));
    assert!(position.castles().has(White, Queen));
    assert_eq!(position.en_passant().map(Into::into), Some(Square::new(E, Three)));
    assert_eq!(position.reversible(), 0);
    assert_eq!(u32::from(position.round()), 1);
    assert_eq!(
        position.validate::<Chess>().unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
    );
    assert_eq!(
        Chess::from_fen(partial_fen).unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR b KQkq - 0 1"
    );

    let board_fen = "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR";
    let position = parse_position.parse(board_fen).unwrap();
    assert_eq!(position.turn(), White);
    assert!(!position.castles().has(Black, King));
    assert!(!position.castles().has(Black, Queen));
    assert!(!position.castles().has(White, King));
    assert!(!position.castles().has(White, Queen));
    assert_eq!(position.en_passant(), None);
    assert_eq!(position.reversible(), 0);
    assert_eq!(u32::from(position.round()), 1);
    assert_eq!(
        position.validate::<Chess>().unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR w - - 0 1"
    );
    assert_eq!(
        Chess::from_fen(board_fen).unwrap().fen(),
        "rnbqkbnr/pp1ppppp/8/2p5/4P3/5N2/PPPP1PPP/RNBQKBNR w - - 0 1"
    );
}

#[test]
fn parses_shredder_castling() {
    use File::*;
    use Player::*;
    use Side::*;

    let fen = "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf - 2 9";
    let position = Freestyle::from_fen(fen).unwrap();
    assert_eq!(position.castles().get(White, King), Some(H));
    assert_eq!(position.castles().get(White, Queen), Some(F));
    assert_eq!(position.castles().get(Black, King), Some(H));
    assert_eq!(position.castles().get(Black, Queen), Some(F));
    assert_eq!(position.fen(), fen);
    assert_eq!(
        position.transposition_fen(),
        "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFhf -"
    );
}

#[test]
fn parses_x_fen_castling() {
    use File::*;
    use Player::*;
    use Side::*;

    let fen = "bbqnnrkr/pppppppp/8/8/8/8/PPPPPPPP/BBQNNRKR w KQkq - 0 1";
    let position = Freestyle::from_fen(fen).unwrap();

    assert_eq!(position.castles().get(White, King), Some(H));
    assert_eq!(position.castles().get(White, Queen), Some(F));
    assert_eq!(position.castles().get(Black, King), Some(H));
    assert_eq!(position.castles().get(Black, Queen), Some(F));
    assert_eq!(position.fen(), "bbqnnrkr/pppppppp/8/8/8/8/PPPPPPPP/BBQNNRKR w HFhf - 0 1");
}

#[test]
fn writes_chess_and_shredder_castling() {
    use File::*;
    use Player::*;
    use Side::*;

    let mut castles = Castles::empty();
    castles.set(White, King, H);
    castles.set(White, Queen, F);
    castles.set(Black, King, H);
    castles.set(Black, Queen, F);

    assert_eq!(castles.chess_fen(), "KQkq");
    assert_eq!(castles.shredder_fen(), "HFhf");
    assert_eq!(castles.fen::<Chess>(), "KQkq");
    assert_eq!(castles.fen::<Freestyle>(), "HFhf");
    assert_eq!(Castles::empty().fen::<Chess>(), "-");
    assert_eq!(Castles::empty().fen::<Freestyle>(), "-");
}

#[test]
fn castle_resolves_x_fen_castling() {
    use File::*;
    use Player::*;
    use Side::*;

    let board = Board::freestyle(crate::position::Scharnagl::new(0).unwrap());
    let rights = castle_rights.parse("KQkq").unwrap();
    let castles = resolve_castles(board, rights).unwrap();

    assert_eq!(castles.get(White, King), Some(H));
    assert_eq!(castles.get(White, Queen), Some(F));
    assert_eq!(castles.get(Black, King), Some(H));
    assert_eq!(castles.get(Black, Queen), Some(F));
}

#[test]
fn castle_resolves_shredder_castling() {
    use File::*;
    use Player::*;
    use Side::*;

    let board = Board::freestyle(crate::position::Scharnagl::new(0).unwrap());
    let rights = castle_rights.parse("HFhf").unwrap();
    let castles = resolve_castles(board, rights).unwrap();

    assert_eq!(castles.get(White, King), Some(H));
    assert_eq!(castles.get(White, Queen), Some(F));
    assert_eq!(castles.get(Black, King), Some(H));
    assert_eq!(castles.get(Black, Queen), Some(F));
}

#[test]
fn rejects_duplicate_castling_files() {
    let fen = "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HH - 2 9";
    assert!(Unvalidated::from_fen(fen).is_err());
}

#[test]
fn rejects_more_than_two_castling_files_per_player() {
    let fen = "bqnb1rkr/pp3ppp/3ppn2/2p5/5P2/P2P4/NPP1P1PP/BQ1BNRKR w HFAh - 2 9";
    assert!(Unvalidated::from_fen(fen).is_err());
}

#[test]
fn board_row_parses_exactly_one_rank() {
    assert!(board_row(Rank::Eight).parse("rnbqkbnr").is_ok());
    assert!(board_row(Rank::Eight).parse("8").is_ok());
}

#[test]
fn board_row_rejects_invalid_rank_width() {
    assert!(board_row(Rank::Eight).parse("7").is_err());
    assert!(board_row(Rank::Eight).parse("9").is_err());
    assert!(board_row(Rank::Eight).parse("rnbqkbnrr").is_err());
    assert!(board_row(Rank::Eight).parse("8r").is_err());
}

#[test]
fn rejects_invalid_board_rank_width() {
    assert!(Unvalidated::from_fen("8/8/8/8/8/8/8/8 w - - 0 1").is_ok());
    assert!(Unvalidated::from_fen("8/8/8/8/8/8/8/7 w - - 0 1").is_err());
    assert!(Unvalidated::from_fen("8/8/8/8/8/8/8/9 w - - 0 1").is_err());
    assert!(Unvalidated::from_fen("8/8/8/8/8/8/8/8r w - - 0 1").is_err());
}
