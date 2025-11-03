use std::io::Read as _;

use chess::formats::{Parser as _, cbv::unpack_cbv_to_disk};

use crate::{Args, Input, Result};

#[derive(Args, Clone, Debug)]
pub struct Command {
    /// Input file (use `-` to read from stdin)
    #[clap(default_value = "-")]
    pub input: Input,
}

impl Command {
    pub fn run(mut self) -> Result<()> {
        let mut input = Vec::new();
        self.input.read_to_end(&mut input)?;

        let header = unpack_cbv_to_disk.parse(&input).map_err(|e| anyhow!("{e:}"))?;

        for entry in header.iter() {
            println!("{}: {} bytes", entry.name, entry.len);
        }
        Ok(())
    }
}
