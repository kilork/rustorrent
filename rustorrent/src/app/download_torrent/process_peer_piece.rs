use super::*;

/// Peer reveived message Have.
pub(crate) async fn process_peer_piece(
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
    peer_piece: usize,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer piece", peer_id);

    let new_pieces = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        match existing_peer.state {
            TorrentPeerState::Connected { .. } => {
                let mut downloadable = vec![];
                let (index, bit) = index_in_bitarray(peer_piece);
                match_pieces(
                    &mut downloadable,
                    &storage.receiver.borrow().downloaded,
                    index,
                    bit,
                );
                downloadable
            }
            TorrentPeerState::Idle | TorrentPeerState::Connecting(_) => {
                error!(
                    "[{}] cannot process peer piece: wrong state: {:?}",
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
