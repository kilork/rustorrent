use super::*;

pub(crate) async fn select_new_peer(
    new_pieces: &[usize],
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
) -> Result<(), RsbtError> {
    for &new_piece in new_pieces {
        if let TorrentDownloadMode::Normal = mode {
            let any_peer_downloading = peer_states.values().any(|x| match x.state {
                TorrentPeerState::Connected {
                    downloading_piece, ..
                } => downloading_piece == Some(new_piece),
                _ => false,
            });
            if any_peer_downloading {
                continue;
            }
        }

        if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
            if let TorrentPeerState::Connected {
                ref mut downloading_piece,
                ref mut downloading_since,
                ref mut sender,
                ..
            } = existing_peer.state
            {
                if downloading_piece.is_none() {
                    *downloading_piece = Some(new_piece);
                    *downloading_since = Some(Instant::now());
                    sender.send(PeerMessage::Download(new_piece)).await?;
                }
            }
        }
    }

    Ok(())
}
