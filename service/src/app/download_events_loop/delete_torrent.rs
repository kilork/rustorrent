use super::*;

pub(crate) async fn delete_torrent(
    request: &RsbtCommandDeleteTorrent,
    torrents: &mut Vec<TorrentDownload>,
) -> Result<(), RsbtError> {
    let id = request.id;

    if let Some(torrent_index) = torrents.iter().position(|x| x.id == id) {
        if let Some(torrent) = torrents.get_mut(torrent_index) {
            torrent.disable().await?;
            torrent.delete(request.files).await?;

            remove_from_current_torrents(torrent.properties.clone(), torrent.header.clone())
                .await?;
        }

        torrents.remove(torrent_index);

        Ok(())
    } else {
        Err(RsbtError::TorrentNotFound(id))
    }
}

impl TorrentDownload {
    async fn delete(&mut self, files: bool) -> Result<(), RsbtError> {
        debug!("delete {}", self.id);

        let (delete_request, response) = RequestResponse::new(files);
        self.process
            .broker_sender
            .clone()
            .send(DownloadTorrentEvent::Delete(delete_request))
            .await?;

        response.await??;

        Ok(())
    }
}
