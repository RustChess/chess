use anyhow::Result;
use chess_cli::Cli;
use clap::Parser as _;

fn main() -> Result<()> {
    Cli::parse().run()
}
