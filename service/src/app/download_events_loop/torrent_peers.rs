use super::*;

pub(crate) async fn torrent_peers(
    request: &RsbtCommandTorrentPeers,
    torrents: &[TorrentDownload],
) -> Result<Vec<RsbtPeerView>, RsbtError> {
    let torrent = find_torrent(torrents, request.id)?;
    torrent.peers().await
}

impl TorrentDownload {
    async fn peers(&self) -> Result<Vec<RsbtPeerView>, RsbtError> {
        debug!("peers for {}", self.id);
        self.request((), DownloadTorrentEvent::PeersView).await
    }
}
