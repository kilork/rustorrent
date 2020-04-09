use super::*;

pub(crate) async fn torrent_file_download(
    request: &RsbtCommandTorrentFileDownload,
    torrents: &[TorrentDownload],
) -> Result<RsbtFileDownloadStream, RsbtError> {
    let torrent = find_torrent(torrents, request.id)?;
    torrent.download_file(request.file_id).await
}

impl TorrentDownload {
    async fn download_file(&self, file_id: usize) -> Result<RsbtFileDownloadStream, RsbtError> {
        debug!("download file {}", self.id);
        self.request(file_id, DownloadTorrentEvent::FileDownload)
            .await
    }
}
