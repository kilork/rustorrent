use crate::peer::PeerMessage;
use std::time::Instant;
use tokio::{sync::mpsc::Sender, task::JoinHandle};

#[derive(Debug)]
pub(crate) enum TorrentPeerState {
    Idle,
    Connecting(JoinHandle<()>),
    Connected {
        chocked: bool,
        interested: bool,
        downloading_piece: Option<usize>,
        downloading_since: Option<Instant>,
        downloaded: usize,
        uploaded: usize,
        sender: Sender<PeerMessage>,
        pieces: Vec<u8>,
    },
}

impl Default for TorrentPeerState {
    fn default() -> Self {
        TorrentPeerState::Idle
    }
}
