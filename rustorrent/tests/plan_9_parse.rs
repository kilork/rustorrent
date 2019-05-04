use rustorrent::types::torrent::Torrent;
use rustorrent::RustorrentError;

use std::convert::TryInto;

#[test]
fn parse_plan_9_torrent() -> Result<(), RustorrentError> {
    let torrent_bytes = include_bytes!("Plan_9_from_Outer_Space_1959_archive.torrent");
    let _: Torrent = torrent_bytes.to_vec().try_into()?;
    Ok(())
}
