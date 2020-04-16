use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq)]
pub enum TorrentProcessStatus {
    Enabled,
    Disabled,
}
