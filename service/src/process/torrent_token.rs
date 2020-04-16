use crate::{
    event::TorrentEvent,
    types::{info::TorrentInfo, Torrent},
    SHA1_SIZE,
};
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub struct TorrentToken {
    pub(crate) torrent: Torrent,
    pub info: TorrentInfo,
    pub(crate) hash_id: [u8; SHA1_SIZE],
    pub(crate) handshake: Vec<u8>,
    pub(crate) broker_sender: Sender<TorrentEvent>,
}
