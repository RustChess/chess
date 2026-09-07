use std::{
    ffi::OsStr,
    fs,
    io::{self, Cursor, IsTerminal as _, Read as _, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use chess::{
    formats::{Parser as _, cbv::unpack_cbv_to, pgn},
    game,
};
use clap::ValueEnum;

use crate::{Args, Result};

type PgnInput = Box<dyn Iterator<Item = io::Result<Result<pgn::Game, pgn::Error>>>>;

#[derive(Args, Clone, Debug)]
pub struct Command {
    /// Input file (use `-` to read from stdin)
    #[clap(short, long, default_value = "-")]
    pub input: PathBuf,

    /// Output file or directory
    #[clap(short, long, conflicts_with = "output_stem")]
    pub output: Option<PathBuf>,

    /// Derive output path from input stem and target format
    #[clap(short = 'O', long)]
    pub output_stem: bool,

    /// Explicitly allow writing text output to terminal stdout
    #[clap(long)]
    pub stdout: bool,

    /// Input format
    #[clap(long)]
    pub from: Option<Format>,

    /// Output format
    pub to: Format,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Format {
    /// PGN text.
    Pgn,
    /// RustChess game archive as JSON.
    Json,
    /// RustChess game archive as CBOR.
    Cbor,
    /// ChessBase archive.
    Cbv,
    /// ChessBase file set / directory.
    Cb,
}

impl Command {
    pub fn run(self) -> Result<()> {
        let from = self.from()?;
        match (from, self.to) {
            (Format::Pgn, Format::Pgn) => self.pgn_to_pgn(),
            (Format::Pgn, Format::Json | Format::Cbor) => {
                let archive = self.pgn_to_archive()?;
                self.archive_to_archive(archive)
            }
            (Format::Json | Format::Cbor, Format::Json | Format::Cbor) => {
                let archive = self.archive_input(from)?;
                self.archive_to_archive(archive)
            }
            (Format::Json | Format::Cbor, Format::Pgn) => {
                let archive = self.archive_input(from)?;
                self.archive_to_pgn(archive)
            }
            (Format::Cbv, Format::Cb) => self.cbv_to_cb(),
            _ => bail!("unsupported conversion: {from:?} -> {:?}", self.to),
        }
    }

    fn from(&self) -> Result<Format> {
        if let Some(from) = self.from {
            return Ok(from);
        }
        if self.input == Path::new("-") {
            bail!("cannot infer input format from stdin; pass --from");
        }
        infer_format(&self.input)
            .with_context(|| format!("cannot infer input format from {}", self.input.display()))
    }

    fn pgn_to_pgn(self) -> Result<()> {
        let mut output = self.output()?;
        let mut first = true;

        for game in self.pgn_input()? {
            let game = game??;
            if !first {
                writeln!(output)?;
                writeln!(output)?;
            }
            write!(output, "{game}")?;
            first = false;
        }

        if !first {
            writeln!(output)?;
        }
        Ok(())
    }

    fn pgn_to_archive(&self) -> Result<game::storage::Archive> {
        let mut archive = game::storage::Archive { games: Vec::new(), plays: Vec::new() };

        for game in self.pgn_input()? {
            let game = game??;
            let game = game::Game::try_from(game)?.store();
            archive.games.extend(game.games);
            archive.plays.extend(game.plays);
        }
        if archive.games.is_empty() {
            bail!("PGN input contains no games");
        }
        Ok(archive)
    }

    fn archive_to_pgn(self, archive: game::storage::Archive) -> Result<()> {
        let game = game::Game::load(archive)?;
        let pgn = pgn::Game::from(game);
        let mut output = self.output()?;
        writeln!(output, "{pgn}")?;
        Ok(())
    }

    fn archive_to_archive(self, archive: game::storage::Archive) -> Result<()> {
        let output = self.output_path_for_directory()?;
        fs::create_dir_all(&output)
            .with_context(|| format!("creating output directory {}", output.display()))?;
        match self.to {
            Format::Json => {
                let mut games = fs::File::create(output.join("game.jsonl"))
                    .with_context(|| format!("writing {}", output.join("game.jsonl").display()))?;
                for game in archive.games {
                    serde_json::to_writer(&mut games, &game)?;
                    writeln!(games)?;
                }

                let mut plays = fs::File::create(output.join("play.jsonl"))
                    .with_context(|| format!("writing {}", output.join("play.jsonl").display()))?;
                for play in archive.plays {
                    serde_json::to_writer(&mut plays, &play)?;
                    writeln!(plays)?;
                }
            }
            Format::Cbor => {
                let mut games =
                    fs::File::create(output.join("game.cbor-seq")).with_context(|| {
                        format!("writing {}", output.join("game.cbor-seq").display())
                    })?;
                for game in archive.games {
                    ciborium::into_writer(&game, &mut games)?;
                }

                let mut plays =
                    fs::File::create(output.join("play.cbor-seq")).with_context(|| {
                        format!("writing {}", output.join("play.cbor-seq").display())
                    })?;
                for play in archive.plays {
                    ciborium::into_writer(&play, &mut plays)?;
                }
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn archive_input(&self, format: Format) -> Result<game::storage::Archive> {
        let input = self.input_path_for_directory()?;
        Ok(match format {
            Format::Json => {
                let games = fs::read_to_string(input.join("game.jsonl"))
                    .with_context(|| format!("reading {}", input.join("game.jsonl").display()))?
                    .lines()
                    .map(serde_json::from_str)
                    .collect::<Result<_, _>>()?;

                let plays = fs::read_to_string(input.join("play.jsonl"))
                    .with_context(|| format!("reading {}", input.join("play.jsonl").display()))?
                    .lines()
                    .map(serde_json::from_str)
                    .collect::<Result<_, _>>()?;

                game::storage::Archive { games, plays }
            }
            Format::Cbor => {
                let games = fs::read(input.join("game.cbor-seq")).with_context(|| {
                    format!("reading {}", input.join("game.cbor-seq").display())
                })?;
                let mut reader = Cursor::new(games.as_slice());
                let mut games = Vec::new();
                while reader.position() < reader.get_ref().len() as u64 {
                    games.push(ciborium::from_reader(&mut reader)?);
                }

                let plays = fs::read(input.join("play.cbor-seq")).with_context(|| {
                    format!("reading {}", input.join("play.cbor-seq").display())
                })?;
                let mut reader = Cursor::new(plays.as_slice());
                let mut plays = Vec::new();
                while reader.position() < reader.get_ref().len() as u64 {
                    plays.push(ciborium::from_reader(&mut reader)?);
                }

                game::storage::Archive { games, plays }
            }
            _ => unreachable!(),
        })
    }

    fn cbv_to_cb(self) -> Result<()> {
        let input = self.read_input()?;
        let output = self.output_path_for_directory()?;
        fs::create_dir_all(&output)
            .with_context(|| format!("creating output directory {}", output.display()))?;
        let header =
            unpack_cbv_to(&output).parse(input.as_slice()).map_err(|error| anyhow!("{error}"))?;
        for entry in header.iter() {
            eprintln!("{}: {} bytes", entry.name, entry.len);
        }
        Ok(())
    }

    fn read_input(&self) -> Result<Vec<u8>> {
        let mut input = Vec::new();
        if self.input == Path::new("-") {
            io::stdin().read_to_end(&mut input)?;
        } else {
            input = fs::read(&self.input)
                .with_context(|| format!("reading input {}", self.input.display()))?;
        }
        Ok(input)
    }

    fn pgn_input(&self) -> Result<PgnInput> {
        if self.input == Path::new("-") {
            Ok(Box::new(pgn::stream::games(io::stdin())))
        } else {
            let file = fs::File::open(&self.input)
                .with_context(|| format!("reading input {}", self.input.display()))?;
            Ok(Box::new(pgn::stream::games(file)))
        }
    }

    fn output(&self) -> Result<Box<dyn io::Write>> {
        if let Some(output) = self.output_path_for_file()? {
            let file = fs::File::create(&output)
                .with_context(|| format!("writing output {}", output.display()))?;
            Ok(Box::new(file))
        } else {
            if io::stdout().is_terminal() && !self.stdout {
                bail!("refusing to write output to terminal stdout");
            }
            Ok(Box::new(io::stdout()))
        }
    }

    fn output_path_for_file(&self) -> Result<Option<PathBuf>> {
        let output =
            if self.output_stem { Some(self.derived_output_path()?) } else { self.output.clone() };
        if let Some(output) = &output {
            self.guard_not_same_path(output)?;
        }
        Ok(output)
    }

    fn output_path_for_directory(&self) -> Result<PathBuf> {
        let output = if self.output_stem || self.output.is_none() {
            self.derived_output_path()?
        } else {
            self.output.clone().expect("checked above")
        };
        self.guard_not_same_path(&output)?;
        Ok(output)
    }

    fn input_path_for_directory(&self) -> Result<&Path> {
        if self.input == Path::new("-") {
            bail!("cannot read directory format from stdin");
        }
        Ok(&self.input)
    }

    fn derived_output_path(&self) -> Result<PathBuf> {
        if self.input == Path::new("-") {
            bail!("-O requires file input");
        }
        let parent = self.input.parent().unwrap_or_else(|| Path::new(""));
        let stem = self.input.file_stem().context("-O requires input with a file stem")?;
        Ok(match self.to {
            Format::Pgn => parent.join(stem).with_extension("pgn"),
            Format::Json | Format::Cbor => parent.join(stem),
            Format::Cb => parent.join(stem),
            Format::Cbv => parent.join(stem).with_extension("cbv"),
        })
    }

    fn guard_not_same_path(&self, output: &Path) -> Result<()> {
        if self.input != Path::new("-") && self.input == output {
            bail!("refusing to overwrite input path {}", output.display());
        }
        Ok(())
    }
}

fn infer_format(path: &Path) -> Result<Format> {
    match path.extension().and_then(OsStr::to_str).map(str::to_ascii_lowercase) {
        Some(extension) if extension == "pgn" => Ok(Format::Pgn),
        Some(extension) if extension == "json" => Ok(Format::Json),
        Some(extension) if extension == "cbor" => Ok(Format::Cbor),
        Some(extension) if extension == "cbv" => Ok(Format::Cbv),
        Some(extension) => bail!("unknown input extension: {extension}"),
        None => bail!("input has no extension"),
    }
}
