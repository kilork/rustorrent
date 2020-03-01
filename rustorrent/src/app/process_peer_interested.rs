use super::*;

pub(crate) async fn process_peer_interested(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
) -> Result<(), RustorrentError> {
    debug!("[{}] process peer interested", peer_id);

    if let Some(TorrentPeerState::Connected {
        ref mut interested, ..
    }) = peer_states.get_mut(&peer_id).map(|x| &mut x.state)
    {
        *interested = true;
    }

    Ok(())
}
