use super::*;

pub(crate) async fn process_announce(
    torrent_process: Arc<TorrentProcess>,
    peers: Vec<Peer>,
) -> Result<(), RsbtError> {
    let mut download_torrent_broker_sender = torrent_process.broker_sender.clone();

    for peer in peers.into_iter().take(1) {
        download_torrent_broker_sender
            .send(DownloadTorrentEvent::PeerAnnounced(peer))
            .await?;
    }

    Ok(())
}
