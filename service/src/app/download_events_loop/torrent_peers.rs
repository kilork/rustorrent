use super::*;

pub(crate) async fn torrent_peers(
    request: &RsbtCommandTorrentPeers,
    torrents: &[TorrentDownload],
) -> Result<Vec<RsbtPeerView>, RsbtError> {
    let id = request.id;

    if let Some(torrent_index) = torrents.iter().position(|x| x.id == id) {
        if let Some(torrent) = torrents.get(torrent_index) {}

        Ok(vec![])
    } else {
        Err(RsbtError::TorrentNotFound(id))
    }
}
