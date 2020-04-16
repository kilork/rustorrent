use crate::process::TorrentProcessHeader;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct CurrentTorrents {
    pub torrents: Vec<TorrentProcessHeader>,
}
