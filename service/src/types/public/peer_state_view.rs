use crate::peer::TorrentPeerState;
use serde::Serialize;
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PeerStateView {
    Idle {},
    Connecting {},
    Connected {
        chocked: bool,
        interested: bool,
        piece: Option<usize>,
        //FIXME: downloading_since: Option<Instant>,
        rx: usize,
        tx: usize,
    },
}

impl From<&TorrentPeerState> for PeerStateView {
    fn from(value: &TorrentPeerState) -> Self {
        match value {
            TorrentPeerState::Idle => Self::Idle {},
            TorrentPeerState::Connecting(_) => Self::Connecting {},
            TorrentPeerState::Connected {
                chocked,
                interested,
                downloading_piece,
                downloading_since,
                downloaded,
                uploaded,
                ..
            } => Self::Connected {
                chocked: *chocked,
                interested: *interested,
                piece: downloading_piece.clone(),
                rx: *downloaded,
                tx: *uploaded,
            },
        }
    }
}
