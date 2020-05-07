use crate::{types::info::TorrentInfo, SHA1_SIZE};

pub(crate) trait TorrentTokenProvider {
    fn info(&self) -> &TorrentInfo;
    fn hash_id(&self) -> &[u8; SHA1_SIZE];
}
