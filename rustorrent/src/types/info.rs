use super::*;
use crate::count_parts;
use flat_storage::FlatStorageFile as TorrentInfoFile;

use std::path::PathBuf;

use crate::{BLOCK_SIZE, SHA1_SIZE};

/// Normalized info from torrent.
#[derive(Debug, PartialEq)]
pub struct TorrentInfo {
    pub piece_length: usize,
    pub default_blocks_count: usize,
    pub last_piece_length: usize,
    pub last_piece_blocks_count: usize,
    pub pieces: Vec<Piece>,
    pub mapping: Vec<PieceToFiles>,
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
            .map(|x| Piece(x.try_into().unwrap()))
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

        let mapping = map_pieces_to_files(piece_length, &files);

        Self {
            piece_length,
            default_blocks_count,
            last_piece_length,
            last_piece_blocks_count,
            pieces,
            mapping,
            length,
            files,
        }
    }
}

fn map_pieces_to_files(piece_length: usize, files: &[TorrentInfoFile]) -> Vec<PieceToFiles> {
    let mut current_piece_left = piece_length;
    let mut current_piece = PieceToFiles(vec![]);
    let mut offset = 0;

    let mut mapping = vec![];

    for (file_index, file) in files.iter().enumerate() {
        let mut file_remaining_length = file.length;
        let mut file_offset = 0;
        while current_piece_left < file_remaining_length {
            current_piece.0.push(FileBlock {
                offset,
                file_index,
                file_offset,
                size: current_piece_left,
            });

            file_remaining_length -= current_piece_left;
            file_offset += current_piece_left;
            current_piece_left = piece_length;

            mapping.push(current_piece);
            current_piece = PieceToFiles(vec![]);
            offset = 0;
        }
        if current_piece_left >= file_remaining_length {
            current_piece.0.push(FileBlock {
                offset,
                file_index,
                file_offset,
                size: file_remaining_length,
            });
            current_piece_left -= file_remaining_length;
            offset += file_remaining_length;
        }
    }

    if !current_piece.0.is_empty() {
        mapping.push(current_piece);
    }

    mapping
}

#[derive(Debug, PartialEq)]
pub struct Piece([u8; SHA1_SIZE]);

impl TryFrom<&[u8]> for Piece {
    type Error = std::array::TryFromSliceError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(Piece(value.try_into()?))
    }
}

#[derive(Debug, PartialEq)]
pub struct PieceToFiles(Vec<FileBlock>);

#[derive(Debug, PartialEq)]
pub struct FileBlock {
    offset: usize,
    file_index: usize,
    file_offset: usize,
    size: usize,
}

#[derive(Debug, PartialEq)]
pub struct TorrentInfoRaw {
    pub name: String,
    pub piece_length: i64,
    pub pieces: Vec<u8>,
    pub length: Option<i64>,
    pub files: Option<Vec<TorrentInfoFileRaw>>,
}

#[derive(Debug, PartialEq)]
pub struct TorrentInfoFileRaw {
    pub length: i64,
    pub path: Vec<String>,
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

try_from_bencode!(TorrentInfoFileRaw,
    normal: (
        "length" => length,
        "path" => path
    ),
);

impl TryFrom<BencodeBlob> for Vec<TorrentInfoFileRaw> {
    type Error = TryFromBencode;

    fn try_from(blob: BencodeBlob) -> Result<Self, Self::Error> {
        match blob.value {
            BencodeValue::List(l) => Ok(l.into_iter().map(|x| x.try_into().unwrap()).collect()),
            _ => Err(TryFromBencode::NotList),
        }
    }
}

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

    #[test]
    fn pieces_to_files() {
        let result = map_pieces_to_files(
            100,
            &[TorrentInfoFile {
                path: "test".into(),
                length: 1000,
            }],
        );
        dbg!(&result);
        assert_eq!(result.len(), 10);

        let result = map_pieces_to_files(
            1000,
            &[TorrentInfoFile {
                path: "test".into(),
                length: 1000,
            }],
        );
        assert_eq!(
            result,
            vec![PieceToFiles(vec![FileBlock {
                offset: 0,
                file_index: 0,
                file_offset: 0,
                size: 1000,
            }])]
        );

        let result = map_pieces_to_files(
            1000,
            &[TorrentInfoFile {
                path: "test".into(),
                length: 800,
            }],
        );
        assert_eq!(
            result,
            vec![PieceToFiles(vec![FileBlock {
                offset: 0,
                file_index: 0,
                file_offset: 0,
                size: 800,
            }])]
        );

        let result = map_pieces_to_files(
            333,
            &[TorrentInfoFile {
                path: "test".into(),
                length: 1000,
            }],
        );
        assert_eq!(
            result,
            vec![
                PieceToFiles(vec![FileBlock {
                    offset: 0,
                    file_index: 0,
                    file_offset: 0,
                    size: 333,
                }]),
                PieceToFiles(vec![FileBlock {
                    offset: 0,
                    file_index: 0,
                    file_offset: 333,
                    size: 333,
                }]),
                PieceToFiles(vec![FileBlock {
                    offset: 0,
                    file_index: 0,
                    file_offset: 666,
                    size: 333,
                }]),
                PieceToFiles(vec![FileBlock {
                    offset: 0,
                    file_index: 0,
                    file_offset: 999,
                    size: 1,
                }])
            ]
        );

        let result = map_pieces_to_files(
            500,
            &[
                TorrentInfoFile {
                    path: "test1".into(),
                    length: 300,
                },
                TorrentInfoFile {
                    path: "test2".into(),
                    length: 400,
                },
                TorrentInfoFile {
                    path: "test3".into(),
                    length: 500,
                },
            ],
        );
        assert_eq!(
            result,
            vec![
                PieceToFiles(vec![
                    FileBlock {
                        offset: 0,
                        file_index: 0,
                        file_offset: 0,
                        size: 300,
                    },
                    FileBlock {
                        offset: 300,
                        file_index: 1,
                        file_offset: 0,
                        size: 200,
                    }
                ]),
                PieceToFiles(vec![
                    FileBlock {
                        offset: 0,
                        file_index: 1,
                        file_offset: 200,
                        size: 200,
                    },
                    FileBlock {
                        offset: 200,
                        file_index: 2,
                        file_offset: 0,
                        size: 300,
                    }
                ]),
                PieceToFiles(vec![FileBlock {
                    offset: 0,
                    file_index: 2,
                    file_offset: 300,
                    size: 200,
                }])
            ]
        );
    }
}
