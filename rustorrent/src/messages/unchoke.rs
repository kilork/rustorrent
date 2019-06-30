use super::*;

pub(crate) fn message_unchoke(
    torrent_process: Arc<TorrentProcess>,
    torrent_peer: Arc<TorrentPeer>,
) -> Result<(), RustorrentError> {
    match *torrent_peer.state.lock().unwrap() {
        TorrentPeerState::Connected {
            ref mut chocked,
            ref interested,
            ref sender,
            ref pieces,
        } => {
            if !*chocked {
                warn!("Peer {}: already unchocked!", torrent_peer.addr);
            }
            *chocked = false;
        }
        _ => (),
    }

    Ok(())
}
