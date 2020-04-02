use super::*;
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentEvent {
    Storage {
        id: usize,
        received: usize,
        uploaded: usize,
    },
    Stat {
        id: usize,
        rx: u64,
        tx: u64,
    },
}
