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

        let (enable_request, response) = RequestResponse::new(());
        self.process
            .broker_sender
            .clone()
            .send(DownloadTorrentEvent::Enable(enable_request))
            .await?;
        response.await??;

        self.update_state(TorrentDownloadStatus::Enabled).await
    }

    async fn disable(&mut self) -> Result<(), RsbtError> {
        debug!("disable {}", self.id);

        let (disable_request, response) = RequestResponse::new(());
        self.process
            .broker_sender
            .clone()
            .send(DownloadTorrentEvent::Disable(disable_request))
            .await?;
        response.await??;
        self.update_state(TorrentDownloadStatus::Disabled).await
    }

    async fn update_state(&mut self, state: TorrentDownloadStatus) -> Result<(), RsbtError> {
        let mut torrent_header = self.header.clone();
        torrent_header.state = state;
        save_current_torrents(self.properties.clone(), torrent_header).await?;

        self.header.state = state;

        Ok(())
    }
}
