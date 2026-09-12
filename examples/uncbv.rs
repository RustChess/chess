use std::{fs, path::Path};

use chess::formats::chessbase::archive;

fn main() -> anyhow::Result<()> {
    let data = fs::read("twic/twic1616.cbv")?;
    let mut input = data.as_slice();
    let header = archive::read_header(&mut input)?;
    for member in &header.members {
        let path = Path::new(".").join(&member.name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = fs::File::create(path)?;
        archive::unpack_to(member, &mut input, &mut output)?;
    }
    println!("{header:?}");
    Ok(())
}
