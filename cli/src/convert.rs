use std::{
    ffi::OsStr,
    fs,
    io::{self, IsTerminal as _, Read as _, Write as _},
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use chess::formats::{Parser as _, cbv::unpack_cbv_to, pgn};
use clap::ValueEnum;

use crate::{Args, Result};

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
        let mut output = self.text_output()?;
        let mut first = true;

        for game in self.pgn_input()? {
            let game = game?.map_err(|error| anyhow!("{error}"))?;
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

    fn pgn_input(
        &self,
    ) -> Result<Box<dyn Iterator<Item = io::Result<chess::formats::ModalResult<pgn::Game>>>>> {
        if self.input == Path::new("-") {
            Ok(Box::new(pgn::read_games(io::stdin())))
        } else {
            let file = fs::File::open(&self.input)
                .with_context(|| format!("reading input {}", self.input.display()))?;
            Ok(Box::new(pgn::read_games(file)))
        }
    }

    fn text_output(&self) -> Result<Box<dyn io::Write>> {
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
        let output = if self.output_stem {
            self.derived_output_path()?
        } else {
            self.output.clone().unwrap_or_else(|| PathBuf::from("."))
        };
        self.guard_not_same_path(&output)?;
        Ok(output)
    }

    fn derived_output_path(&self) -> Result<PathBuf> {
        if self.input == Path::new("-") {
            bail!("-O requires file input");
        }
        let parent = self.input.parent().unwrap_or_else(|| Path::new(""));
        let stem = self.input.file_stem().context("-O requires input with a file stem")?;
        Ok(match self.to {
            Format::Pgn => parent.join(stem).with_extension("pgn"),
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
        Some(extension) if extension == "cbv" => Ok(Format::Cbv),
        Some(extension) => bail!("unknown input extension: {extension}"),
        None => bail!("input has no extension"),
    }
}
