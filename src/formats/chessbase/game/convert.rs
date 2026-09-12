use crate::{
    Player, Side,
    board::{Board, Role},
    game::Node,
    position::{Castles as ChessCastles, EnPassant, Parts, Special},
    square::{File, Rank, Square},
};

use super::{Game, Move, PawnMove, Pieces, Position, Token};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("custom ChessBase starting positions are not supported yet")]
    Start,
    #[error("ChessBase null moves are not supported")]
    Null,
    #[error("missing ChessBase {player} {role} number {number}")]
    Piece { player: Player, role: Role, number: u8 },
    #[error("invalid ChessBase move from {from} by ({file}, {rank})")]
    Offset { from: Square, file: i8, rank: i8 },
    #[error("invalid ChessBase move from {from} to {to}")]
    Move { from: Square, to: Square },
    #[error(transparent)]
    Game(#[from] crate::game::Error),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

impl TryFrom<Game> for crate::Game {
    type Error = Error;

    fn try_from(encoded: Game) -> Result<Self> {
        Self::from_chessbase(encoded)
    }
}

impl crate::Game {
    pub fn from_chessbase(encoded: Game) -> Result<Self> {
        let Game { position, tokens } = encoded;
        let mut pieces = position.pieces;
        let mut game = Self::from(crate::Position::try_from(position)?);
        game.decode_tokens(&mut tokens.into_iter(), &mut pieces, Node::Start)?;
        Ok(game)
    }

    fn decode_tokens(
        &mut self,
        tokens: &mut impl Iterator<Item = Token>,
        pieces: &mut Pieces,
        mut previous: Node,
    ) -> Result<()> {
        while let Some(token) = tokens.next() {
            match token {
                Token::Move(encoded) => {
                    let position = match previous {
                        Node::Start => self.start(),
                        Node::Play(slot) => {
                            self.play(slot).expect("decoded predecessor exists").position()
                        }
                    };
                    let play = pieces.resolve(position, encoded)?;
                    pieces.apply(position, play);
                    previous = Node::Play(
                        self.options_mut(previous)
                            .expect("decoded predecessor exists")
                            .push(play)?
                            .slot(),
                    );
                }
                Token::Push => {
                    let saved = *pieces;
                    self.decode_tokens(tokens, pieces, previous)?;
                    *pieces = saved;
                }
                Token::Pop => return Ok(()),
                Token::Skip => {}
            }
        }
        Ok(())
    }
}

impl TryFrom<Position> for crate::Position {
    type Error = Error;

    fn try_from(encoded: Position) -> Result<Self> {
        let mut board = Board::EMPTY;
        for player in Player::iter() {
            for role in Role::iter() {
                for square in encoded.pieces[player][role].into_iter().flatten() {
                    if board.get(square).is_some() {
                        return Err(Error::Start);
                    }
                    board.insert(square, role.of(player));
                }
            }
        }

        let mut castles = ChessCastles::empty();
        for player in Player::iter() {
            for side in Side::iter() {
                if encoded.castles[player][side] {
                    let king = board.king_of(player).ok_or(Error::Start)?;
                    let mut rooks = board.rooks().intersection(board.player(player));
                    let file = loop {
                        let rook = rooks.pop_first().ok_or(Error::Start)?;
                        if rook.rank() == player.backrank()
                            && Side::of_rook(king, rook.file()) == side
                        {
                            break rook.file();
                        }
                    };
                    castles.set(player, side, file);
                }
            }
        }

        let en_passant = encoded.en_passant.and_then(|file| {
            let rank = if encoded.turn.is_white() { Rank::Six } else { Rank::Three };
            EnPassant::from_square(Square::new(file, rank))
        });
        Parts {
            board,
            turn: encoded.turn,
            castles,
            en_passant,
            reversible: 0,
            round: encoded.round,
        }
        .validate()
        .map_err(|_| Error::Start)
    }
}

impl Pieces {
    fn resolve(&self, position: crate::Position, encoded: Move) -> Result<crate::Move> {
        use Move::*;
        use PawnMove::*;

        let player = position.turn();
        let (from, to, promotion) = match encoded {
            Null => return Err(Error::Null),
            Castle(dx) => {
                let from = self.piece(player, Role::King, 0)?;
                let side = if dx > 0 { Side::King } else { Side::Queen };
                let to = player.castle_king_to(side);
                return position
                    .legal_moves()
                    .into_iter()
                    .find(|play| {
                        play.from == from
                            && play
                                .kind
                                .castle_rook_file()
                                .is_some_and(|file| Side::of_rook(from, file) == side)
                    })
                    .ok_or(Error::Move { from, to });
            }
            King(dx, dy) => {
                let from = self.piece(player, Role::King, 0)?;
                let to =
                    checked_add(from, dx, dy).ok_or(Error::Offset { from, file: dx, rank: dy })?;
                (from, to, None)
            }
            Queen(number, dx, dy) => {
                let from = self.piece(player, Role::Queen, number)?;
                (from, wrapping_add(from, dx, dy), None)
            }
            Rook(number, dx, dy) => {
                let from = self.piece(player, Role::Rook, number)?;
                (from, wrapping_add(from, dx, dy), None)
            }
            Bishop(number, dx, dy) => {
                let from = self.piece(player, Role::Bishop, number)?;
                (from, wrapping_add(from, dx, dy), None)
            }
            Knight(number, dx, dy) => {
                let from = self.piece(player, Role::Knight, number)?;
                let to =
                    checked_add(from, dx, dy).ok_or(Error::Offset { from, file: dx, rank: dy })?;
                (from, to, None)
            }
            Pawn(file, movement) => {
                let from = self.piece(player, Role::Pawn, file as u8)?;
                let dy = if player.is_white() { 1 } else { -1 };
                let (dx, dy) = match movement {
                    One => (0, dy),
                    Two => (0, 2 * dy),
                    Right => (dy, dy),
                    Left => (-dy, dy),
                };
                let to =
                    checked_add(from, dx, dy).ok_or(Error::Offset { from, file: dx, rank: dy })?;
                (from, to, None)
            }
            FromTo(from, to, role) => {
                let promotion =
                    (position.board().role_at(from) == Some(Role::Pawn)).then_some(role);
                (from, to, promotion)
            }
        };

        position
            .legal_moves()
            .into_iter()
            .find(|play| {
                play.from == from
                    && play.to == to
                    && match promotion {
                        Some(role) => play.kind.is_promote_role(role),
                        None => !play.kind.is_promote(),
                    }
            })
            .ok_or(Error::Move { from, to })
    }

    fn piece(&self, player: Player, role: Role, number: u8) -> Result<Square> {
        self[player][role].get(number as usize).copied().flatten().ok_or(Error::Piece {
            player,
            role,
            number,
        })
    }

    fn apply(&mut self, position: crate::Position, play: crate::Move) {
        let player = position.turn();
        let board = position.board();
        let captured = if play.kind.is_en_passant() {
            Some(Square::new(play.to.file(), play.from.rank()))
        } else {
            board.get(play.to).map(|_| play.to)
        };
        if let Some(square) = captured {
            let piece = board.get(square).expect("captured square contains a piece");
            self.capture(piece.player, piece.role, square);
        }

        self.move_piece(player, play.role, play.from, play.to);
        match play.kind.specials() {
            Some(Special::Castle(file)) => {
                let side = crate::Side::of_rook(play.from, file);
                let from = player.castle_rook_from(file);
                let to = player.castle_rook_to(side);
                self.move_piece(player, Role::Rook, from, to);
            }
            Some(Special::Promote(role)) => {
                self.remove(player, Role::Pawn, play.to);
                let promoted = self[player][role]
                    .iter_mut()
                    .find(|square| square.is_none())
                    .expect("ChessBase supports at most ten pieces of one role");
                *promoted = Some(play.to);
            }
            Some(Special::EnPassant) | None => {}
        }
    }

    fn move_piece(&mut self, player: Player, role: Role, from: Square, to: Square) {
        let square = self[player][role]
            .iter_mut()
            .find(|square| **square == Some(from))
            .expect("resolved ChessBase piece identity exists");
        *square = Some(to);
    }

    fn remove(&mut self, player: Player, role: Role, square: Square) {
        let pieces = &mut self[player][role];
        let index = pieces
            .iter()
            .position(|candidate| *candidate == Some(square))
            .expect("captured ChessBase piece identity exists");
        pieces[index] = None;
    }

    fn capture(&mut self, player: Player, role: Role, square: Square) {
        let pieces = &mut self[player][role];
        let index = pieces
            .iter()
            .position(|candidate| *candidate == Some(square))
            .expect("captured ChessBase piece identity exists");
        if role == Role::Pawn {
            pieces[index] = None;
        } else {
            pieces[index..].rotate_left(1);
            *pieces.last_mut().unwrap() = None;
        }
    }
}

fn checked_add(from: Square, file: i8, rank: i8) -> Option<Square> {
    let file = u8::try_from(from.file() as i16 + i16::from(file)).ok()?;
    let rank = u8::try_from(from.rank() as i16 + i16::from(rank)).ok()?;
    Some(Square::new(File::from_index(file)?, Rank::from_index(rank)?))
}

fn wrapping_add(from: Square, file: u8, rank: u8) -> Square {
    let file = (u16::from(from.file() as u8) + u16::from(file)) % 8;
    let rank = (u16::from(from.rank() as u8) + u16::from(rank)) % 8;
    Square::new(File::panicky_from_index(file as u8), Rank::panicky_from_index(rank as u8))
}
