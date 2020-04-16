use crate::process::TorrentProcessStatus;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentProcessHeader {
    pub file: String,
    pub state: TorrentProcessStatus,
}
