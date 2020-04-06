use super::*;

pub(crate) async fn process_announce(
    torrent_process: Arc<TorrentProcess>,
    peers: Vec<Peer>,
) -> Result<(), RsbtError> {
    let mut download_torrent_broker_sender = torrent_process.broker_sender.clone();

    for peer in peers {
        download_torrent_broker_sender
            .send(DownloadTorrentEvent::PeerAnnounced(peer))
            .await?;

        delay_for(Duration::from_secs(1)).await;
    }

    Ok(())
}
