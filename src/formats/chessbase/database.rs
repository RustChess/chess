//! Access to ChessBase databases.

use std::{
    fs::File,
    io::{self, Cursor, Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    formats::{Text, prelude::*},
    game::{self as native, Tag},
};

use super::{
    archive::{self, Access, Kind, Member},
    game, index, player, table, tournament,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid ChessBase database")]
    Data,
    #[error(transparent)]
    Archive(#[from] archive::Error),
    #[error(transparent)]
    Convert(#[from] game::convert::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl From<FrameError<ContextMode>> for Error {
    fn from(error: FrameError<ContextMode>) -> Self {
        match error {
            FrameError::Error(_) => Self::Data,
            FrameError::Io(error) => Self::Io(error),
        }
    }
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

/// A ChessBase database backed by a reader.
pub struct Database<R: Reader> {
    pub reader: R,
}

/// Input containing ChessBase game data.
pub trait Reader {
    type Entries<'a>: Iterator<Item = Result<Entry>>
    where
        Self: 'a;

    fn entries(&mut self) -> Result<Self::Entries<'_>>;
}

/// Input containing a ChessBase game index as well as game data.
pub trait IndexReader: Reader {
    fn entry(&mut self, index: usize) -> Result<Option<Entry>>;
}

/// A ChessBase archive.
pub struct Archive<I> {
    pub index: Access<I>,
    pub data: Access<I>,
    pub members: Vec<Member>,
    pub games_header: game::Header,
    pub index_header: index::Header,
    pub players_header: Option<table::Header>,
    pub tournaments_header: Option<table::Header>,
}

/// A standalone ChessBase game file.
pub struct Games<I> {
    pub input: I,
    pub header: game::Header,
}

/// A ChessBase game and index file bundle, with optional metadata tables.
pub struct Bundle<I> {
    pub games: I,
    pub games_header: game::Header,
    pub index: I,
    pub index_header: index::Header,
    pub players: Option<I>,
    pub players_header: Option<table::Header>,
    pub tournaments: Option<I>,
    pub tournaments_header: Option<table::Header>,
}

/// Input accepted by a ChessBase database reader.
pub trait ReadSeek: Read + Seek {}

/// One game and its available index-derived data.
pub struct Entry {
    pub game: game::Game,
    pub index: Option<Indexed>,
}

/// Data joined to a game through its ChessBase index entry.
pub struct Indexed {
    pub index: index::Game,
    pub black: Option<player::Player>,
    pub tournament: Option<tournament::Tournament>,
    pub white: Option<player::Player>,
}

/// Iterator over standalone ChessBase game records.
pub struct Entries<'a, I> {
    input: &'a mut I,
    body: Vec<u8>,
    count: usize,
    end: u64,
    failed: bool,
}

/// Iterator over indexed ChessBase game records.
pub struct BundleEntries<'a, I> {
    games: &'a mut I,
    index: &'a mut I,
    index_len: usize,
    players: Option<(&'a mut I, table::Header)>,
    tournaments: Option<(&'a mut I, table::Header)>,
    body: Vec<u8>,
    count: usize,
    failed: bool,
}

/// Iterator over archived ChessBase game records.
pub struct ArchiveEntries<'a, I> {
    index: archive::Reader<'a, I>,
    data: &'a mut Access<I>,
    index_len: usize,
    players_header: Option<table::Header>,
    tournaments_header: Option<table::Header>,
    body: Vec<u8>,
    count: usize,
    failed: bool,
}

impl<R: Reader> Database<R> {
    pub fn games(&mut self) -> Result<impl Iterator<Item = Result<crate::Game>> + '_> {
        Ok(self.reader.entries()?.map(|entry| entry.and_then(Entry::into_game)))
    }
}

impl<R: IndexReader> Database<R> {
    pub fn game(&mut self, index: usize) -> Result<Option<crate::Game>> {
        self.reader.entry(index)?.map(Entry::into_game).transpose()
    }
}

impl Entry {
    fn into_game(self) -> Result<crate::Game> {
        let mut game = crate::Game::from_chessbase(self.game)?;
        if let Some(indexed) = self.index {
            indexed.enrich(&mut game);
        }
        Ok(game)
    }
}

impl Indexed {
    fn enrich(self, game: &mut crate::Game) {
        let Self { index, black, tournament, white } = self;

        game.roster.white = white.and_then(player_name);
        game.roster.black = black.and_then(player_name);
        game.roster.date = date(index.date);
        game.roster.round = round(index.round, index.subround);
        game.outcome = match index.outcome {
            index::Outcome::WhiteWins => native::Outcome::White,
            index::Outcome::BlackWins => native::Outcome::Black,
            index::Outcome::Draw => native::Outcome::Draw,
            _ => native::Outcome::Unknown,
        };

        if let Some(white_elo) = index.white_elo {
            game.tags.push(tag("WhiteElo", white_elo));
        }
        if let Some(black_elo) = index.black_elo {
            game.tags.push(tag("BlackElo", black_elo));
        }
        if let Some(tournament) = tournament {
            game.roster.event = Text::new(tournament.title).ok();
            game.roster.site = Text::new(tournament.place).ok();
            if let Some(date) = date(tournament.date) {
                game.tags.push(Tag {
                    key: Text::new("EventDate").expect("tag name is non-empty"),
                    value: date.to_string(),
                });
            }
        }
    }
}

fn player_name(player: player::Player) -> Option<Text> {
    let name = match (player.last_name.is_empty(), player.first_name.is_empty()) {
        (false, false) => format!("{},{}", player.last_name, player.first_name),
        (false, true) => player.last_name,
        (true, false) => player.first_name,
        (true, true) => return None,
    };
    Text::new(name).ok()
}

fn date(date: index::Date) -> Option<Text> {
    (date.year.is_some() || date.month.is_some() || date.day.is_some())
        .then(|| Text::new(date.pgn()).expect("formatted date is non-empty"))
}

fn round(round: Option<u8>, subround: Option<u8>) -> Option<Text> {
    match (round, subround) {
        (Some(round), Some(subround)) => Text::new(format!("{round}.{subround}")).ok(),
        (Some(round), None) => Text::new(round.to_string()).ok(),
        (None, Some(subround)) => Text::new(format!("?.{subround}")).ok(),
        (None, None) => None,
    }
}

fn tag(key: &'static str, value: impl ToString) -> Tag {
    Tag { key: Text::new(key).expect("tag name is non-empty"), value: value.to_string() }
}

impl<T: Read + Seek> ReadSeek for T {}

impl<I: ReadSeek> Reader for Games<I> {
    type Entries<'a>
        = Entries<'a, I>
    where
        Self: 'a;

    fn entries(&mut self) -> Result<Self::Entries<'_>> {
        self.input
            .seek(SeekFrom::Start(u64::from(self.header.first_game)))
            .map_err(|_| Error::Data)?;

        Ok(Entries {
            input: &mut self.input,
            body: Vec::new(),
            count: 0,
            end: u64::from(self.header.len),
            failed: false,
        })
    }
}

impl<I: ReadSeek> Reader for Bundle<I> {
    type Entries<'a>
        = BundleEntries<'a, I>
    where
        Self: 'a;

    fn entries(&mut self) -> Result<Self::Entries<'_>> {
        self.entries_from(0)
    }
}

impl<I: ReadSeek> IndexReader for Bundle<I> {
    fn entry(&mut self, index: usize) -> Result<Option<Entry>> {
        if index >= self.index_header.len {
            return Ok(None);
        }
        self.entries_from(index)?.next().transpose()
    }
}

impl<I: ReadSeek> Reader for Archive<I> {
    type Entries<'a>
        = ArchiveEntries<'a, I>
    where
        Self: 'a;

    fn entries(&mut self) -> Result<Self::Entries<'_>> {
        self.entries_from(0)
    }
}

impl<I: ReadSeek> IndexReader for Archive<I> {
    fn entry(&mut self, index: usize) -> Result<Option<Entry>> {
        if index >= self.index_header.len {
            return Ok(None);
        }
        self.entries_from(index)?.next().transpose()
    }
}

impl<I: ReadSeek> Iterator for Entries<'_, I> {
    type Item = Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }

        let result = self.next_entry();
        if result.is_err() {
            self.failed = true;
        }
        result.transpose()
    }
}

impl<I: ReadSeek> Entries<'_, I> {
    fn next_entry(&mut self) -> Result<Option<Entry>> {
        // CBG reserves 1 KiB for growth after every 100 physical game records.
        if self.count > 0 && self.count.is_multiple_of(100) {
            self.input.seek(SeekFrom::Current(1024)).map_err(|_| Error::Data)?;
        }

        let position = self.input.stream_position().map_err(|_| Error::Data)?;
        if position == self.end {
            return Ok(None);
        }
        if position > self.end {
            return Err(Error::Data);
        }

        let game = framed(self.input, &mut self.body, game::info, game::game)?;
        let end = self.input.stream_position().map_err(|_| Error::Data)?;
        if end > self.end {
            return Err(Error::Data);
        }
        self.count += 1;

        Ok(Some(Entry { game, index: None }))
    }
}

impl<I: ReadSeek> Iterator for BundleEntries<'_, I> {
    type Item = Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.count == self.index_len {
            return None;
        }

        let result = self.next_entry();
        if result.is_err() {
            self.failed = true;
        }
        Some(result)
    }
}

impl<I: ReadSeek> BundleEntries<'_, I> {
    fn next_entry(&mut self) -> Result<Entry> {
        let indexed = read_index(self.index)?;
        let game = read_game(self.games, indexed.offsets.data, &mut self.body)?;

        let white = match self.players.as_mut() {
            Some((input, header)) => {
                Some(read_table(input, *header, indexed.offsets.white, player::player)?)
            }
            None => None,
        };
        let black = match self.players.as_mut() {
            Some((input, header)) => {
                Some(read_table(input, *header, indexed.offsets.black, player::player)?)
            }
            None => None,
        };
        let tournament = match self.tournaments.as_mut() {
            Some((input, header)) => Some(read_table(
                input,
                *header,
                indexed.offsets.tournament,
                tournament::tournament,
            )?),
            None => None,
        };

        self.count += 1;
        Ok(Entry { game, index: Some(Indexed { index: indexed, black, tournament, white }) })
    }
}

impl<I: ReadSeek> Iterator for ArchiveEntries<'_, I> {
    type Item = Result<Entry>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.count == self.index_len {
            return None;
        }

        let result = self.next_entry();
        if result.is_err() {
            self.failed = true;
        }
        Some(result)
    }
}

impl<I: ReadSeek> ArchiveEntries<'_, I> {
    fn next_entry(&mut self) -> Result<Entry> {
        let indexed = read_index(&mut self.index)?;
        let game = {
            let mut games = self.data.reader(Kind::Games).ok_or(Error::Data)?;
            read_game(&mut games, indexed.offsets.data, &mut self.body)?
        };
        let white = match self.players_header {
            Some(header) => {
                let mut players = self.data.reader(Kind::Players).ok_or(Error::Data)?;
                Some(read_table(&mut players, header, indexed.offsets.white, player::player)?)
            }
            None => None,
        };
        let black = match self.players_header {
            Some(header) => {
                let mut players = self.data.reader(Kind::Players).ok_or(Error::Data)?;
                Some(read_table(&mut players, header, indexed.offsets.black, player::player)?)
            }
            None => None,
        };
        let tournament = match self.tournaments_header {
            Some(header) => {
                let mut tournaments = self.data.reader(Kind::Tournaments).ok_or(Error::Data)?;
                Some(read_table(
                    &mut tournaments,
                    header,
                    indexed.offsets.tournament,
                    tournament::tournament,
                )?)
            }
            None => None,
        };

        self.count += 1;
        Ok(Entry { game, index: Some(Indexed { index: indexed, black, tournament, white }) })
    }
}

fn read_index(input: &mut impl ReadSeek) -> Result<index::Game> {
    let mut bytes = [0; index::Game::LEN];
    input.read_exact(&mut bytes).map_err(|_| Error::Data)?;
    index::game.parse(bytes.as_slice()).map_err(|_| Error::Data)
}

fn read_game(input: &mut impl ReadSeek, offset: usize, body: &mut Vec<u8>) -> Result<game::Game> {
    input.seek(SeekFrom::Start(offset as u64)).map_err(|_| Error::Data)?;
    Ok(framed(input, body, game::info, game::game)?)
}

fn read_table<I: ReadSeek, T>(
    input: &mut I,
    header: table::Header,
    index: usize,
    mut parser: impl FnMut(&mut &[u8]) -> winnow::ModalResult<T>,
) -> Result<T> {
    if index > header.len {
        return Err(Error::Data);
    }
    let len = header.record_len + 9;
    let offset = header
        .first_record
        .checked_add(index.checked_mul(len).ok_or(Error::Data)?)
        .ok_or(Error::Data)?;
    input.seek(SeekFrom::Start(offset as u64)).map_err(|_| Error::Data)?;
    let mut bytes = vec![0; len];
    input.read_exact(&mut bytes).map_err(|_| Error::Data)?;
    parser(&mut bytes.as_slice()).map_err(|_| Error::Data)
}

impl Archive<File> {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(
            File::open(path.as_ref()).map_err(|_| Error::Data)?,
            File::open(path).map_err(|_| Error::Data)?,
        )
    }
}

impl<'a> Archive<Cursor<&'a [u8]>> {
    pub fn from_slice(input: &'a [u8]) -> Result<Self> {
        Self::new(Cursor::new(input), Cursor::new(input))
    }
}

impl<I: ReadSeek> Archive<I> {
    fn entries_from(&mut self, count: usize) -> Result<ArchiveEntries<'_, I>> {
        let mut index = self.index.reader(Kind::Index).ok_or(Error::Data)?;
        index.seek(SeekFrom::Start(index_offset(count)?)).map_err(|_| Error::Data)?;

        Ok(ArchiveEntries {
            index,
            data: &mut self.data,
            index_len: self.index_header.len,
            players_header: self.players_header,
            tournaments_header: self.tournaments_header,
            body: Vec::new(),
            count,
            failed: false,
        })
    }

    pub fn new(mut index: I, data: I) -> Result<Self> {
        index.rewind().map_err(|_| Error::Data)?;
        let header = archive::read_header(&mut index)?;
        let members = header.members;
        let mut index = Access::new(index, &members)?;
        let mut data = Access::new(data, &members)?;
        let games_header = {
            let mut games = data.reader(Kind::Games).ok_or(Error::Data)?;
            read_header::<{ game::Header::LEN }, _, _>(&mut games, game::header)?
        };
        let index_header = {
            let mut input = index.reader(Kind::Index).ok_or(Error::Data)?;
            read_header::<{ index::Header::LEN }, _, _>(&mut input, index::header)?
        };
        let players_header = match data.reader(Kind::Players) {
            Some(mut players) => Some(read_table_header(&mut players)?),
            None => None,
        };
        let tournaments_header = match data.reader(Kind::Tournaments) {
            Some(mut tournaments) => Some(read_table_header(&mut tournaments)?),
            None => None,
        };
        Ok(Self {
            index,
            data,
            members,
            games_header,
            index_header,
            players_header,
            tournaments_header,
        })
    }
}

impl<I: ReadSeek> Games<I> {
    pub fn new(mut input: I) -> Result<Self> {
        input.rewind().map_err(|_| Error::Data)?;
        let mut bytes = [0; 26];
        input.read_exact(&mut bytes).map_err(|_| Error::Data)?;
        let header = game::header.parse(bytes.as_slice()).map_err(|_| Error::Data)?;
        Ok(Self { input, header })
    }
}

impl Games<File> {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::new(File::open(path).map_err(|_| Error::Data)?)
    }
}

impl<'a> Games<Cursor<&'a [u8]>> {
    pub fn from_slice(input: &'a [u8]) -> Result<Self> {
        Self::new(Cursor::new(input))
    }
}

impl<I: ReadSeek> Bundle<I> {
    fn entries_from(&mut self, count: usize) -> Result<BundleEntries<'_, I>> {
        self.index.seek(SeekFrom::Start(index_offset(count)?)).map_err(|_| Error::Data)?;

        Ok(BundleEntries {
            games: &mut self.games,
            index: &mut self.index,
            index_len: self.index_header.len,
            players: self.players.as_mut().zip(self.players_header),
            tournaments: self.tournaments.as_mut().zip(self.tournaments_header),
            body: Vec::new(),
            count,
            failed: false,
        })
    }

    pub fn new(mut games: I, mut index: I) -> Result<Self> {
        let games_header = read_header::<{ game::Header::LEN }, _, _>(&mut games, game::header)?;
        let index_header = read_header::<{ index::Header::LEN }, _, _>(&mut index, index::header)?;
        Ok(Self {
            games,
            games_header,
            index,
            index_header,
            players: None,
            players_header: None,
            tournaments: None,
            tournaments_header: None,
        })
    }

    fn set_players(&mut self, mut input: I) -> Result<()> {
        self.players_header = Some(read_table_header(&mut input)?);
        self.players = Some(input);
        Ok(())
    }

    fn set_tournaments(&mut self, mut input: I) -> Result<()> {
        self.tournaments_header = Some(read_table_header(&mut input)?);
        self.tournaments = Some(input);
        Ok(())
    }
}

impl Bundle<File> {
    pub fn from_files(games: impl AsRef<Path>, index: impl AsRef<Path>) -> Result<Self> {
        Self::new(
            File::open(games).map_err(|_| Error::Data)?,
            File::open(index).map_err(|_| Error::Data)?,
        )
    }

    pub fn with_players(mut self, players: impl AsRef<Path>) -> Result<Self> {
        self.set_players(File::open(players).map_err(|_| Error::Data)?)?;
        Ok(self)
    }

    pub fn with_tournaments(mut self, tournaments: impl AsRef<Path>) -> Result<Self> {
        self.set_tournaments(File::open(tournaments).map_err(|_| Error::Data)?)?;
        Ok(self)
    }
}

impl<'a> Bundle<Cursor<&'a [u8]>> {
    pub fn from_slices(games: &'a [u8], index: &'a [u8]) -> Result<Self> {
        Self::new(Cursor::new(games), Cursor::new(index))
    }

    pub fn with_players(mut self, players: &'a [u8]) -> Result<Self> {
        self.set_players(Cursor::new(players))?;
        Ok(self)
    }

    pub fn with_tournaments(mut self, tournaments: &'a [u8]) -> Result<Self> {
        self.set_tournaments(Cursor::new(tournaments))?;
        Ok(self)
    }
}

fn read_header<const N: usize, I: ReadSeek, T>(
    input: &mut I,
    mut parser: impl FnMut(&mut &[u8]) -> winnow::ModalResult<T>,
) -> Result<T> {
    input.rewind().map_err(|_| Error::Data)?;
    let mut bytes = [0; N];
    input.read_exact(&mut bytes).map_err(|_| Error::Data)?;
    parser(&mut bytes.as_slice()).map_err(|_| Error::Data)
}

fn read_table_header(input: &mut impl ReadSeek) -> Result<table::Header> {
    input.rewind().map_err(|_| Error::Data)?;
    let mut bytes = [0; 32];
    input.read_exact(&mut bytes).map_err(|_| Error::Data)?;
    table::header.parse_next(&mut bytes.as_slice()).map_err(|_| Error::Data)
}

fn index_offset(index: usize) -> Result<u64> {
    let offset = index
        .checked_mul(index::Game::LEN)
        .and_then(|offset| offset.checked_add(index::Header::LEN))
        .ok_or(Error::Data)?;
    u64::try_from(offset).map_err(|_| Error::Data)
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::formats::twic;

    use super::{archive::Range, *};

    const GAME: [u8; 109] = hex!(
        "0000006d0b08dc870210c19a0a11008182882659a72344d4124253447cf81804
         98bb07014e172835883cab0772e32431097526317d4384464d3e0a1490fc4c78
         fa61f88cb815f05b46c97dc02979f1e2462aafd3c9de2f9d06aa9914c7ffa818
         54e1b5db120fc36c1c9b9f2674"
    );
    const INDEX_GAME: [u8; index::Game::LEN] = hex!(
        "010000001a000000000000000000010000000000000000010fd355000003250a
         240ac7cd80000000000000000034"
    );
    const WHITE: [u8; 67] = hex!(
        "7e010000ffffffffff4368656e6700686f007200006c000ade01000000000080
         000000009e8600426f007400617370726f00646f736f00000000000500000001
         000000"
    );
    const BLACK: [u8; 67] = hex!(
        "ffffffffffffffff00476972690069204a007300657200007900790079007900
         000000008f86004100616e656d006c696e61006e6f7300010000000500000001
         000000"
    );
    const TOURNAMENT: [u8; 99] = hex!(
        "ffffffffffffffff003430746820454343204f70656e20323032350032303235
         0000009f0dde010000e8f9e176de01000052686f646573204752450052410000
         74204155540001000080d8c0b9f87f53d30f0000013700000007005802000001
         000000"
    );
    const GAMES_HEADER: [u8; game::Header::LEN] =
        hex!("001a000000870000000000000000000000870000000000000000");
    const INDEX_HEADER: [u8; index::Header::LEN] = hex!(
        "00002c002e050000000200000000000000000000000000000000000000000000
         0000000000000000000000020000"
    );

    fn games() -> Vec<u8> {
        let mut input = GAMES_HEADER.to_vec();
        input.extend(GAME);
        input
    }

    fn index() -> Vec<u8> {
        let mut input = INDEX_HEADER.to_vec();
        input.extend(INDEX_GAME);
        input
    }

    fn table(records: &[&[u8]], record_len: u32) -> Vec<u8> {
        let len = u32::try_from(records.len() - 1).expect("compact table length fits u32");
        let mut input = Vec::new();
        input.extend(len.to_le_bytes());
        input.extend(0u32.to_le_bytes());
        input.extend(1_234_567_890u32.to_le_bytes());
        input.extend(record_len.to_le_bytes());
        input.extend((-1i32).to_le_bytes());
        input.extend(u32::try_from(records.len()).unwrap().to_le_bytes());
        input.extend(0u32.to_le_bytes());
        for record in records {
            input.extend_from_slice(record);
        }
        input
    }

    #[test]
    fn games_from_slice() {
        let input = games();
        let mut games = Games::from_slice(&input).unwrap();
        assert_eq!(games.entries().unwrap().count(), 1);

        let first = games.entries().unwrap().next().expect("first CBG game");
        first.expect("can reread first CBG game");
    }

    #[test]
    fn bundle_from_slices() {
        let games = games();
        let index = index();
        let players = table(&[&WHITE, &BLACK], 58);
        let tournaments = table(&[&TOURNAMENT], 90);
        let mut bundle = Bundle::from_slices(&games, &index)
            .unwrap()
            .with_players(&players)
            .unwrap()
            .with_tournaments(&tournaments)
            .unwrap();

        let entry = bundle.entries().unwrap().next().expect("first indexed game").unwrap();
        let indexed = entry.index.expect("index data");
        assert_eq!(indexed.index.offsets.data, 26);
        assert!(indexed.white.is_some());
        assert!(indexed.black.is_some());
        assert!(indexed.tournament.is_some());

        let mut database = Database { reader: bundle };
        assert!(database.game(0).unwrap().is_some());
        assert!(database.game(1).unwrap().is_none());
        let game = database.games().unwrap().next().expect("first game").unwrap();
        assert!(game.roster.white.is_some());
        assert!(game.roster.black.is_some());
        assert!(game.roster.event.is_some());
        assert!(game.roster.site.is_some());
    }

    #[test]
    #[ignore = "exhaustive test against the external TWIC fixture"]
    fn twic_archive_from_slice() {
        let cbv = twic::data(1616, "cbv");
        let mut archive = Archive::from_slice(&cbv).unwrap();
        let index = archive
            .members
            .iter()
            .find(|member| member.name.ends_with(".cbh"))
            .expect("CBH member");
        let games = archive
            .members
            .iter()
            .find(|member| member.name.ends_with(".cbg"))
            .expect("CBG member");

        assert_eq!(index.packed, Range::from(2430..130450));
        assert_eq!(games.packed, Range::from(131267..724157));

        let unpacked_len = games.len;
        let mut games = archive.data.reader(Kind::Games).expect("CBG reader");
        let mut unpacked = Vec::new();
        games.read_to_end(&mut unpacked).unwrap();
        assert_eq!(unpacked.len(), unpacked_len);

        let mut database = Database { reader: archive };
        assert!(database.game(0).unwrap().is_some());
        let game = database.games().unwrap().next().expect("first archived game").unwrap();
        assert!(game.roster.white.is_some());
        assert!(game.roster.black.is_some());
        assert!(game.roster.event.is_some());
        assert!(game.roster.site.is_some());
    }

    #[test]
    #[ignore = "exhaustive test against the external TWIC fixture"]
    fn twic_archive_seek() {
        let cbv = twic::data(1616, "cbv");
        let expected = twic::data(1616, "cbg");
        let mut archive = Archive::from_slice(&cbv).unwrap();
        let mut games = archive.data.reader(Kind::Games).expect("CBG reader");

        for position in [0, 61_400, 122_900, 70_000, 300_000, 100, expected.len() - 257] {
            games.seek(SeekFrom::Start(position as u64)).unwrap();
            let mut actual = [0; 257];
            games.read_exact(&mut actual).unwrap();
            assert_eq!(actual.as_slice(), &expected[position..position + actual.len()]);
        }
    }

    #[test]
    #[ignore = "exhaustive comparison against the external TWIC fixture"]
    fn twic_archive_bundle() {
        let cbv = twic::data(1616, "cbv");
        let cbg = twic::data(1616, "cbg");
        let cbh = twic::data(1616, "cbh");
        let cbp = twic::data(1616, "cbp");
        let cbt = twic::data(1616, "cbt");
        let archive = Archive::from_slice(cbv.as_slice()).unwrap();
        let bundle = Bundle::from_slices(cbg.as_slice(), cbh.as_slice())
            .unwrap()
            .with_players(cbp.as_slice())
            .unwrap()
            .with_tournaments(cbt.as_slice())
            .unwrap();
        let mut archive = Database { reader: archive };
        let mut bundle = Database { reader: bundle };
        let mut archive = archive.games().unwrap();
        let mut bundle = bundle.games().unwrap();
        let mut count = 0;

        loop {
            match (archive.next(), bundle.next()) {
                (Some(archive), Some(bundle)) => {
                    let archive = archive.unwrap_or_else(|_| panic!("archived game {count}"));
                    let bundle = bundle.unwrap_or_else(|_| panic!("bundled game {count}"));
                    assert!(archive == bundle, "archived and bundled game {count} differ");
                    count += 1;
                }
                (None, None) => break,
                _ => panic!("archive and bundle lengths differ after game {count}"),
            }
        }
        assert_eq!(count, 6251);
    }

    #[test]
    #[ignore = "test against the external TWIC fixture"]
    fn twic_archive_from_file() {
        let archive = Archive::from_file(twic::path(1616, "cbv")).unwrap();
        let mut database = Database { reader: archive };
        database.games().unwrap().next().expect("first archived game").unwrap();
    }
}
