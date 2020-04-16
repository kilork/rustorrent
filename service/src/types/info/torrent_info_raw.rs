use crate::SHA1_SIZE;
use crate::{
    types::{info::TorrentInfoFileRaw, BencodeBlob},
    RsbtError,
};
use std::convert::{TryFrom, TryInto};

#[derive(Debug, PartialEq)]
pub struct TorrentInfoRaw {
    pub name: String,
    pub piece_length: i64,
    pub pieces: Vec<u8>,
    pub length: Option<i64>,
    pub files: Option<Vec<TorrentInfoFileRaw>>,
}

impl TorrentInfoRaw {
    /// Returns total length of torrent in bytes.
    ///
    /// For single file torrent it is the size of this file.
    /// For multi files torrent it is the sum of all file sizes.
    pub fn len(&self) -> usize {
        if let Some(len) = self.length {
            len as usize
        } else if let Some(files) = &self.files {
            files.iter().map(|x| x.length).sum::<i64>() as usize
        } else {
            panic!("Wrong torrent info block");
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Count of pieces in torrent.
    pub fn pieces_count(&self) -> usize {
        self.pieces.len() / SHA1_SIZE
    }

    /// Piece by index.
    pub fn piece(&self, index: usize) -> Option<&[u8]> {
        let index = index * SHA1_SIZE;
        self.pieces.get(index..index + SHA1_SIZE)
    }
}

try_from_bencode!(TorrentInfoRaw,
    normal: (
        "name" => name,
        "piece length" => piece_length,
        "pieces" => pieces
    ),
    optional: (
        "length" => length,
        "files" => files
    ),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces() {
        let torrent_info = TorrentInfoRaw {
            name: "torrent_info".into(),
            piece_length: 10,
            pieces: b"a123456789b123456789c123456789d123456789".to_vec(),
            length: Some(100),
            files: None,
        };
        assert_eq!(torrent_info.pieces_count(), 2);
        assert_eq!(
            torrent_info.piece(0),
            Some(b"a123456789b123456789".as_ref())
        );
        assert_eq!(
            torrent_info.piece(1),
            Some(b"c123456789d123456789".as_ref())
        );
        assert_eq!(torrent_info.piece(2), None);
    }
}
