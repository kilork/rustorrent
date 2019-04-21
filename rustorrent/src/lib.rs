mod errors;
mod parser;
mod types;

use std::convert::TryInto;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use errors::RustorrentError;
use types::Torrent;

pub fn parse_torrent(filename: impl AsRef<Path>) -> Result<(), RustorrentError> {
    let mut f = File::open(filename)?;

    let mut buf = vec![];
    f.read_to_end(&mut buf)?;

    let torrent: Torrent = parser::parse_bencode(&buf).try_into()?;

    dbg!(&torrent);

    Ok(())
}
