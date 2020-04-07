use super::*;

pub(crate) async fn torrent_pieces(
    request: &RsbtCommandTorrentPieces,
    torrents: &[TorrentDownload],
) -> Result<Vec<u8>, RsbtError> {
    let torrent = find_torrent(torrents, request.id)?;
    Ok(torrent.storage_state_watch.borrow().downloaded.to_vec())
}
