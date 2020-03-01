use super::*;

pub(crate) async fn process_peer_piece_canceled(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer piece downloaded", peer_id);

    let new_pieces = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        if let TorrentPeerState::Connected {
            ref pieces,
            ref mut downloading_piece,
            ref mut downloading_since,
            ..
        } = existing_peer.state
        {
            *downloading_piece = None;
            *downloading_since = None;
            let mut downloadable = vec![];
            for (i, &a) in pieces.iter().enumerate() {
                match_pieces(
                    &mut downloadable,
                    &storage.receiver.borrow().downloaded,
                    i,
                    a,
                );
            }
            downloadable
        } else {
            return Ok(());
        }
    } else {
        return Ok(());
    };

    select_new_peer(&new_pieces, peer_states, mode, peer_id, storage).await?;

    Ok(())
}
