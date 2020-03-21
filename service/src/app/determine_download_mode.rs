use super::*;

pub(crate) fn determine_download_mode(
    peer_states: &mut HashMap<Uuid, PeerState>,
    storage: &mut TorrentStorage,
    peer_id: Uuid,
) -> TorrentDownloadMode {
    let pieces_left = storage.receiver.borrow().pieces_left;

    let connected_count = peer_states
        .values()
        .filter(|x| match x.state {
            TorrentPeerState::Connected { .. } => true,
            _ => false,
        })
        .count();

    let final_mode = (pieces_left as usize) < connected_count;

    if final_mode {
        debug!("[{}] select piece in final mode", peer_id);
        TorrentDownloadMode::Final
    } else {
        debug!("[{}] select piece in normal mode", peer_id);
        TorrentDownloadMode::Normal
    }
}
