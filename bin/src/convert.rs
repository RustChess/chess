use std::{
    ffi::OsStr,
    fs,
    io::{self, IsTerminal as _, Read as _, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use chess::formats::{
    Pgn,
    chessbase::{archive, database},
    pgn,
};
use clap::ValueEnum;

use crate::{Args, Result};

type PgnInput = Box<dyn Iterator<Item = io::Result<Result<Pgn, pgn::Error>>>>;

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
            (Format::Cbv, Format::Pgn) => self.cbv_to_pgn(),
            (Format::Cbv, Format::Cb) => self.cbv_to_cb(),
            (Format::Cb, Format::Pgn) => self.cb_to_pgn(),
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

    fn cbv_to_cb(self) -> Result<()> {
        let mut input: Box<dyn io::Read> = if self.input == Path::new("-") {
            Box::new(io::stdin())
        } else {
            Box::new(
                fs::File::open(&self.input)
                    .with_context(|| format!("reading input {}", self.input.display()))?,
            )
        };
        let output = self.output_path_for_directory()?;
        fs::create_dir_all(&output)
            .with_context(|| format!("creating output directory {}", output.display()))?;
        let header = archive::read_header(&mut input)?;
        for member in &header.members {
            let path = output.join(&member.name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating output directory {}", parent.display()))?;
            }
            let mut file = fs::File::create(&path)
                .with_context(|| format!("writing output {}", path.display()))?;
            archive::unpack_to(member, &mut input, &mut file)
                .with_context(|| format!("unpacking {}", member.name))?;
            info!("{}: {} bytes", member.name, member.len);
        }
        Ok(())
    }

    fn cbv_to_pgn(self) -> Result<()> {
        let mut output = self.output()?;
        if self.input == Path::new("-") {
            let input = self.read_input()?;
            let archive = database::Archive::from_slice(&input)?;
            write_pgns(database::Database { reader: archive }, &mut output)
        } else {
            let archive = database::Archive::from_file(&self.input)
                .with_context(|| format!("reading input {}", self.input.display()))?;
            write_pgns(database::Database { reader: archive }, &mut output)
        }
    }

    fn cb_to_pgn(self) -> Result<()> {
        if self.input == Path::new("-") {
            bail!("cannot read ChessBase file set from stdin");
        }

        let mut output = self.output()?;
        let index = self.input.with_extension("cbh");
        if !index.try_exists()? {
            let games = database::Games::from_file(&self.input)
                .with_context(|| format!("reading input {}", self.input.display()))?;
            return write_pgns(database::Database { reader: games }, &mut output);
        }

        let mut bundle = database::Bundle::from_files(&self.input, &index)
            .with_context(|| format!("reading input {}", self.input.display()))?;
        let players = self.input.with_extension("cbp");
        if players.try_exists()? {
            bundle = bundle.with_players(players)?;
        }
        let tournaments = self.input.with_extension("cbt");
        if tournaments.try_exists()? {
            bundle = bundle.with_tournaments(tournaments)?;
        }
        write_pgns(database::Database { reader: bundle }, &mut output)
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
            Ok(Box::new(pgn::stream::pgns(io::stdin())))
        } else {
            let file = fs::File::open(&self.input)
                .with_context(|| format!("reading input {}", self.input.display()))?;
            Ok(Box::new(pgn::stream::pgns(file)))
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
        let output = if self.output_stem {
            Some(self.derived_output_path()?)
        } else if let Some(output) = &self.output {
            Some(if output.is_dir() {
                output.join(self.derived_output_name()?)
            } else {
                output.clone()
            })
        } else {
            None
        };
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

    fn derived_output_path(&self) -> Result<PathBuf> {
        if self.input == Path::new("-") {
            bail!("-O requires file input");
        }
        let parent = self.input.parent().unwrap_or_else(|| Path::new(""));
        Ok(parent.join(self.derived_output_name()?))
    }

    fn derived_output_name(&self) -> Result<PathBuf> {
        if self.input == Path::new("-") {
            bail!("cannot derive output name from stdin");
        }
        let stem = self.input.file_stem().context("-O requires input with a file stem")?;
        Ok(match self.to {
            Format::Pgn => PathBuf::from(stem).with_extension("pgn"),
            Format::Cb => PathBuf::from(stem),
            Format::Cbv => PathBuf::from(stem).with_extension("cbv"),
        })
    }

    fn guard_not_same_path(&self, output: &Path) -> Result<()> {
        if self.input != Path::new("-") && self.input == output {
            bail!("refusing to overwrite input path {}", output.display());
        }
        Ok(())
    }
}

fn write_pgns<R: database::Reader>(
    mut database: database::Database<R>,
    output: &mut dyn io::Write,
) -> Result<()> {
    let mut first = true;
    for game in database.games()? {
        if !first {
            writeln!(output)?;
            writeln!(output)?;
        }
        write!(output, "{}", Pgn::from(game?))?;
        first = false;
    }
    if !first {
        writeln!(output)?;
    }
    Ok(())
}

fn infer_format(path: &Path) -> Result<Format> {
    match path.extension().and_then(OsStr::to_str).map(str::to_ascii_lowercase) {
        Some(extension) if extension == "pgn" => Ok(Format::Pgn),
        Some(extension) if extension == "cbv" => Ok(Format::Cbv),
        Some(extension) if extension == "cbg" => Ok(Format::Cb),
        Some(extension) => bail!("unknown input extension: {extension}"),
        None => bail!("input has no extension"),
    }
}
