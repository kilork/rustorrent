use super::*;

pub(crate) async fn process_peer_unchoke(
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
) -> Result<(), RsbtError> {
    debug!("[{}] process peer unchoke", peer_id);

    if let Some(TorrentPeerState::Connected {
        ref mut chocked, ..
    }) = peer_states.get_mut(&peer_id).map(|x| &mut x.state)
    {
        *chocked = false;
    }

    Ok(())
}
