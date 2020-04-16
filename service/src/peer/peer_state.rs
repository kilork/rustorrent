use crate::{peer::TorrentPeerState, types::Peer};

#[derive(Debug)]
pub(crate) struct PeerState {
    pub(crate) peer: Peer,
    pub(crate) state: TorrentPeerState,
    pub(crate) announce_count: usize,
}
