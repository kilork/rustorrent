use super::*;
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentEvent {
    Storage { id: usize, write: u64, read: u64 },
    Stat { id: usize, rx: u64, tx: u64 },
}
