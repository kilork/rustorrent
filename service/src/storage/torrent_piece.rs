use std::ops::Deref;

#[derive(Debug, Clone)]
pub struct TorrentPiece(pub(crate) Vec<u8>);

impl Deref for TorrentPiece {
    type Target = dyn AsRef<[u8]>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
