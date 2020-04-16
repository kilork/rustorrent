use serde::Serialize;
use serde_with::skip_serializing_none;

#[skip_serializing_none]
#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentStatisticsEvent {
    Storage {
        id: usize,
        write: u64,
        read: u64,
        left: u32,
    },
    Stat {
        id: usize,
        rx: u64,
        tx: u64,
    },
}
