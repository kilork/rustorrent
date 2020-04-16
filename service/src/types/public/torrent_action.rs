use serde::{Deserialize, Serialize};

#[serde(rename_all = "lowercase")]
#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum TorrentAction {
    Enable,
    Disable,
}
