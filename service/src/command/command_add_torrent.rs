use crate::process::TorrentProcessStatus;

#[derive(Debug)]
pub struct CommandAddTorrent {
    pub data: Vec<u8>,
    pub filename: String,
    pub state: TorrentProcessStatus,
}
