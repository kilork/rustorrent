use super::*;

pub(crate) async fn torrent_file_download(
    request: &RsbtCommandTorrentFileDownload,
    torrents: &[TorrentDownload],
) -> Result<RsbtFileDownloadStream, RsbtError> {
    let RsbtCommandTorrentFileDownload { id, file_id, range } = request;
    let torrent = find_torrent(torrents, *id)?;
    torrent.download_file(*file_id).await
}

impl TorrentDownload {
    async fn download_file(&self, file_id: usize) -> Result<RsbtFileDownloadStream, RsbtError> {
        debug!("download file {}", self.id);
        self.request(file_id, DownloadTorrentEvent::FileDownload)
            .await
    }
}
