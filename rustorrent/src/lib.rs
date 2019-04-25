mod errors;
mod parser;
pub mod types;

use std::convert::TryInto;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use errors::RustorrentError;
use types::Torrent;

pub fn parse_torrent<'a>(
    filename: impl AsRef<Path>,
    buf: &'a mut std::vec::Vec<u8>,
) -> Result<Torrent, RustorrentError> {
    let mut f = File::open(filename)?;

    f.read_to_end(buf)?;

    let torrent = buf.as_slice().try_into()?;

    Ok(torrent)
}
