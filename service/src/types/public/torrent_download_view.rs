use crate::process::{TorrentProcess, TorrentProcessStatus};
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct TorrentDownloadView {
    pub id: usize,
    pub name: String,
    pub write: u64,
    pub read: u64,
    pub tx: u64,
    pub rx: u64,
    pub pieces_total: u32,
    pub pieces_left: u32,
    pub piece_size: u32,
    pub length: usize,
    pub active: bool,
}

impl From<&TorrentProcess> for TorrentDownloadView {
    fn from(torrent: &TorrentProcess) -> Self {
        let (read, write, pieces_left) = {
            let storage_state = torrent.storage_state_watch.borrow();
            (
                storage_state.bytes_read,
                storage_state.bytes_write,
                storage_state.pieces_left,
            )
        };
        let (tx, rx) = {
            let state = torrent.statistics_watch.borrow();
            (state.uploaded, state.downloaded)
        };
        Self {
            id: torrent.id,
            name: torrent.name.clone(),
            active: torrent.header.state == TorrentProcessStatus::Enabled,
            length: torrent.process.info.length,
            write,
            read,
            tx,
            rx,
            pieces_left,
            pieces_total: torrent.process.info.pieces.len() as u32,
            piece_size: torrent.process.info.piece_length as u32,
        }
    }
}
// FIXME: rename to TorrentProcessView
