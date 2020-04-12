use super::*;

pub(crate) async fn torrent_files(
    request: &RsbtCommandTorrentFiles,
    torrents: &[TorrentDownload],
) -> Result<Vec<RsbtFileView>, RsbtError> {
    let torrent = find_torrent(torrents, request.id)?;
    torrent.files().await
}

pub(crate) async fn torrent_file(
    request: &RsbtCommandTorrentFileDownload,
    torrents: &[TorrentDownload],
) -> Result<RsbtFileView, RsbtError> {
    let RsbtCommandTorrentFileDownload { id, file_id, range } = request;
    let torrent = find_torrent(torrents, *id)?;
    let files = torrent.files().await?;
    let file_id = *file_id;
    files
        .into_iter()
        .find(|x| x.id == file_id)
        .ok_or_else(|| RsbtError::TorrentFileNotFound(file_id))
}

impl TorrentDownload {
    async fn files(&self) -> Result<Vec<RsbtFileView>, RsbtError> {
        debug!("files for {}", self.id);
        self.request((), DownloadTorrentEvent::FilesView).await
    }
}
