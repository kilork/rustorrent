use super::*;

pub(crate) async fn torrent_announces(
    request: &RsbtCommandTorrentAnnounce,
    torrents: &[TorrentDownload],
) -> Result<Vec<RsbtAnnounceView>, RsbtError> {
    let torrent = find_torrent(torrents, request.id)?;
    torrent.announces().await
}

impl TorrentDownload {
    async fn announces(&self) -> Result<Vec<RsbtAnnounceView>, RsbtError> {
        debug!("peers for {}", self.id);
        self.request((), DownloadTorrentEvent::AnnounceView).await
    }
}
