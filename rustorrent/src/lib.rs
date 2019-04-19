pub mod errors;
mod parser;

use std::io::{Result, Read};
use std::fs::File;
use std::path::Path;

pub fn parse_torrent(filename: impl AsRef<Path>) -> Result<()> {
    let mut f = File::open(filename)?;
    Ok(())
}