use super::*;

pub(crate) fn message_choke(
    torrent_process: Arc<TorrentProcess>,
    torrent_peer: Arc<TorrentPeer>,
) -> Result<Option<RustorrentCommand>, RustorrentError> {
    info!("Processing message to choke peer: {}", &torrent_peer.addr);
    let mut state = torrent_peer.state.lock().unwrap();
    match *state {
        TorrentPeerState::Connected {
            ref mut chocked, ..
        } => {
            if *chocked {
                warn!("Peer {}: already choked!", torrent_peer.addr);
            }
            *chocked = true;
        }
        _ => warn!("Peer {} is in wrong state: {:?}", torrent_peer.addr, state),
    }

    Ok(None)
}
