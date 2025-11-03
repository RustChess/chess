#[macro_use(anyhow)]
extern crate anyhow;

use anyhow::Result;
use clap::{Args, CommandFactory as _, Parser, Subcommand};
use clio::Input;

mod convert;

#[derive(Clone, Debug, Parser)]
#[clap(bin_name = "chess", name = "chess", version = clap::crate_version!())]
#[clap(infer_subcommands = true)]
pub struct Cli {
    #[clap(flatten)]
    pub options: Options,
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Args, Clone, Debug)]
pub struct Options {
    // #[clap(flatten)]
    // pub verbosity: clap_verbosity_flag::Verbosity,
}

#[derive(Clone, Debug, Subcommand)]
pub enum Command {
    Convert(convert::Command),
    Version(Version),
}

/// Print version
#[derive(Args, Clone, Debug)]
pub struct Version;

impl Version {
    fn run(self) -> Result<()> {
        let cmd = Cli::command();
        let (name, version) = (cmd.get_name(), cmd.get_version().unwrap());
        println!("{name} {version}");
        Ok(())
    }
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Convert(cmd) => cmd.run(),
            Command::Version(cmd) => cmd.run(),
        }
    }
}
