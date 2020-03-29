use super::*;

pub(crate) async fn torrent_action(
    properties: Arc<Properties>,
    action: &RsbtCommandTorrentAction,
    torrents: &mut Vec<TorrentDownload>,
) -> Result<(), RsbtError> {
    let id = action.id;

    if let Some(torrent) = torrents.iter().find(|x| x.id == id) {
        Ok(())
    } else {
        Err(RsbtError::TorrentNotFound(id))
    }
}
