use super::*;

pub(crate) async fn process_peer_pieces(
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
    peer_pieces: Vec<u8>,
    storage: &mut TorrentStorage,
) -> Result<(), RsbtError> {
    debug!("[{}] process peer pieces", peer_id);

    let new_pieces = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        match &mut existing_peer.state {
            TorrentPeerState::Connected { pieces, .. } => collect_pieces_and_update(
                pieces,
                &peer_pieces,
                &storage.receiver.borrow().downloaded,
            ),
            TorrentPeerState::Idle | TorrentPeerState::Connecting(_) => {
                error!(
                    "[{}] cannot process peer pieces: wrong state: {:?}",
                    peer_id, existing_peer.state
                );
                vec![]
            }
        }
    } else {
        vec![]
    };

    select_new_peer(&new_pieces, peer_states, mode, peer_id).await?;

    Ok(())
}
