use super::*;

pub(crate) async fn torrent_detail(
    request: &RsbtCommandTorrentDetail,
    torrents: &[TorrentDownload],
) -> Result<TorrentDownloadView, RsbtError> {
    let torrent = find_torrent(torrents, request.id)?;
    Ok(torrent.into())
}
