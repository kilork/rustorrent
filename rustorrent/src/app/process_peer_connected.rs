use super::*;

pub(crate) async fn process_peer_connected(
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
    stream: TcpStream,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer connection initiated", peer_id);

    if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        let (sender, receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let _ = spawn_and_log_error(
            peer_loop(torrent_process, peer_id, sender.clone(), receiver, stream),
            move || format!("[{}] existing peer loop failed", peer_id),
        );

        existing_peer.state = TorrentPeerState::Connected {
            chocked: true,
            interested: false,
            downloading_piece: None,
            downloading_since: None,
            downloaded: 0,
            uploaded: 0,
            pieces: vec![],
            sender,
        };
    }

    Ok(())
}
