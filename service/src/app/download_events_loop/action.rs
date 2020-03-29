use super::*;

pub(crate) async fn torrent_action(
    request: &RsbtCommandTorrentAction,
    torrents: &mut Vec<TorrentDownload>,
) -> Result<(), RsbtError> {
    let id = request.id;

    if let Some(torrent) = torrents.iter_mut().find(|x| x.id == id) {
        match request.action {
            RsbtTorrentAction::Enable => torrent.enable().await,
            RsbtTorrentAction::Disable => torrent.disable().await,
        }
    } else {
        Err(RsbtError::TorrentNotFound(id))
    }
}

impl TorrentDownload {
    async fn enable(&mut self) -> Result<(), RsbtError> {
        debug!("enable {}", self.id);
        // FIXME: send events to announce and peers to start them

        self.update_state(TorrentDownloadState::Enabled).await
    }

    async fn disable(&mut self) -> Result<(), RsbtError> {
        debug!("disable {}", self.id);
        // FIXME: send events to announce and peers to stop them

        self.update_state(TorrentDownloadState::Disabled).await
    }

    async fn update_state(&mut self, state: TorrentDownloadState) -> Result<(), RsbtError> {
        let mut torrent_header = self.header.clone();
        torrent_header.state = state;
        save_current_torrents(self.properties.clone(), torrent_header).await?;

        self.header.state = state;

        Err(RsbtError::TorrentActionNotSupported)
    }
}
