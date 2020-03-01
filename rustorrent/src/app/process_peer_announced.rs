use super::*;

pub(crate) async fn process_peer_announced(
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer: Peer,
) -> Result<(), RustorrentError> {
    let mut peer_states_iter = peer_states.iter_mut();
    let peer_err = peer.clone();
    if let Some((peer_id, existing_peer)) = peer_states_iter.find(|x| x.1.peer == peer) {
        let peer_id = peer_id.clone();
        match existing_peer.state {
            TorrentPeerState::Idle => {
                let handler = spawn_and_log_error(
                    connect_to_peer(torrent_process, peer_id, peer),
                    move || format!("connect to existing peer {} {:?} failed", peer_id, peer_err),
                );
                existing_peer.state = TorrentPeerState::Connecting(handler);
            }
            TorrentPeerState::Connected { .. } => {
                existing_peer.announce_count += 1;
            }
            _ => (),
        }
    } else {
        let peer_id = Uuid::new_v4();
        let torrent_process_on_failure = torrent_process.clone();
        peer_states.insert(
            peer_id,
            PeerState {
                peer: peer.clone(),
                state: TorrentPeerState::Connecting(tokio::spawn(async move {
                    if let Err(err) = connect_to_peer(torrent_process, peer_id, peer).await {
                        error!(
                            "[{}] connect to new peer {:?} failed: {}",
                            peer_id, peer_err, err
                        );
                        if let Err(err) = torrent_process_on_failure
                            .broker_sender
                            .clone()
                            .send(DownloadTorrentEvent::PeerConnectFailed(peer_id))
                            .await
                        {
                            error!("[{}] cannot send peer connect failed: {}", peer_id, err);
                        }
                    }
                })),
                announce_count: 0,
            },
        );
    };

    Ok(())
}
