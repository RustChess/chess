use chess::formats::{Parser as _, cbv::unpack_cbv_to_disk};

pub const EXAMPLE: &[u8] = include_bytes!("twic1616.cbv");
// pub const EXAMPLE: &[u8] = include_bytes!("../twic1617.cbv");
// pub const EXAMPLE: &[u8] = include_bytes!("../TWIC_DB_1__1617.cbv");

fn main() -> anyhow::Result<()> {
    let header = unpack_cbv_to_disk.parse(EXAMPLE).map_err(|e| anyhow::format_err!("{e}"))?;
    println!("{header:?}");
    Ok(())
}
