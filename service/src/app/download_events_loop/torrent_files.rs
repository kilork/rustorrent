use super::*;

pub(crate) async fn torrent_files(
    request: &RsbtCommandTorrentFiles,
    torrents: &[TorrentDownload],
) -> Result<Vec<RsbtFileView>, RsbtError> {
    let torrent = find_torrent(torrents, request.id)?;
    torrent.files().await
}

impl TorrentDownload {
    async fn files(&self) -> Result<Vec<RsbtFileView>, RsbtError> {
        debug!("files for {}", self.id);
        self.request((), DownloadTorrentEvent::FilesView).await
    }
}
