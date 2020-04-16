use crate::count_parts;
use crate::types::info::{PieceChecksum, TorrentInfoFileRaw, TorrentInfoRaw};
use crate::{BLOCK_SIZE, SHA1_SIZE};
use flat_storage::FlatStorageFile as TorrentInfoFile;
use serde::Deserialize;
use std::convert::TryInto;

/// Normalized info from torrent.
#[derive(Debug, PartialEq, Clone, Deserialize)]
pub struct TorrentInfo {
    pub piece_length: usize,
    pub default_blocks_count: usize,
    pub last_piece_length: usize,
    pub last_piece_blocks_count: usize,
    pub pieces: Vec<PieceChecksum>,
    pub length: usize,
    pub files: Vec<TorrentInfoFile>,
}

impl TorrentInfo {
    /// Returns total length of torrent in bytes.
    ///
    /// For single file torrent it is the size of this file.
    /// For multi files torrent it is the sum of all file sizes.
    pub fn len(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns piece length and blocks count from piece index.
    /// For last piece information can differ, for that reason we need piece index.
    pub fn sizes(&self, index: usize) -> (usize, usize) {
        let is_last_piece = index != self.pieces.len() - 1;

        if is_last_piece {
            (self.piece_length, self.default_blocks_count)
        } else {
            (self.last_piece_length, self.last_piece_blocks_count)
        }
    }
}

impl From<TorrentInfoRaw> for TorrentInfo {
    fn from(raw: TorrentInfoRaw) -> Self {
        let pieces = raw
            .pieces
            .as_slice()
            .chunks_exact(SHA1_SIZE)
            .map(|x| PieceChecksum(x.try_into().unwrap()))
            .collect();

        let length = raw.len();

        let files = if let Some(length) = raw.length.map(|x| x.try_into().unwrap()) {
            vec![TorrentInfoFile {
                path: raw.name.into(),
                length,
            }]
        } else if let Some(files) = raw.files {
            files
                .iter()
                .map(|TorrentInfoFileRaw { path, length }| TorrentInfoFile {
                    path: path.iter().collect(),
                    length: *length as usize,
                })
                .collect()
        } else {
            panic!();
        };

        let piece_length = raw.piece_length as usize;

        let default_blocks_count = count_parts(piece_length, BLOCK_SIZE);

        let mut last_piece_length = length % piece_length;
        if last_piece_length == 0 {
            last_piece_length = piece_length;
        }

        let last_piece_blocks_count = count_parts(last_piece_length, BLOCK_SIZE);

        Self {
            piece_length,
            default_blocks_count,
            last_piece_length,
            last_piece_blocks_count,
            pieces,
            length,
            files,
        }
    }
}
