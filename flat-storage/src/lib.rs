use failure::Fail;
use serde::{Deserialize, Serialize};
use std::{ops::Deref, path::PathBuf};

#[inline]
pub fn index_in_bitarray(index: usize) -> (usize, u8) {
    (index / 8, 128 >> (index % 8))
}

#[inline]
pub fn bit_by_index(index: usize, data: &[u8]) -> Option<(usize, u8)> {
    let (index_byte, index_bit) = index_in_bitarray(index);
    data.get(index_byte).and_then(|&v| {
        if v & index_bit == index_bit {
            Some((index_byte, index_bit))
        } else {
            None
        }
    })
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct FlatStorageFile {
    pub path: PathBuf,
    pub length: usize,
}

pub struct FlatStoragePieceIndex(usize);

impl From<usize> for FlatStoragePieceIndex {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl Deref for FlatStoragePieceIndex {
    type Target = usize;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Flat storage of different sized files.
///
/// Access to data is asynchronious. Files created lazy.
pub trait FlatStorage {
    fn files(&self) -> &[FlatStorageFile];

    fn read_piece<I: Into<FlatStoragePieceIndex>>(
        &self,
        index: I,
    ) -> Result<Option<Vec<u8>>, FlatStorageError>;

    fn write_piece<I: Into<FlatStoragePieceIndex>>(
        &self,
        index: I,
        block: Vec<u8>,
    ) -> Result<(), FlatStorageError>;
}

#[derive(Debug, Fail)]
pub enum FlatStorageError {
    #[fail(display = "cannot allocate file")]
    AllocateFile,
    #[fail(display = "cannot read block from file")]
    ReadBlock,
    #[fail(display = "cannot write block to file")]
    WriteBlock,
}

#[cfg(test)]
mod tests {
    use super::{bit_by_index, index_in_bitarray};

    #[test]
    fn checks_index_in_bitarray() {
        assert_eq!((0, 0b10000000), index_in_bitarray(0));
        assert_eq!((0, 0b01000000), index_in_bitarray(1));
        assert_eq!((0, 0b00100000), index_in_bitarray(2));
        assert_eq!((0, 0b00010000), index_in_bitarray(3));
        assert_eq!((0, 0b00001000), index_in_bitarray(4));
        assert_eq!((0, 0b00000100), index_in_bitarray(5));
        assert_eq!((0, 0b00000010), index_in_bitarray(6));
        assert_eq!((0, 0b00000001), index_in_bitarray(7));
        assert_eq!((1, 0b10000000), index_in_bitarray(8));
    }

    #[test]
    fn checks_bit_by_index() {
        assert_eq!(None, bit_by_index(0, &[]));
        assert_eq!(None, bit_by_index(0, &[0b0000_0000]));
        assert_eq!(Some((0, 0b10000000)), bit_by_index(0, &[0b1000_0000]));
        assert_eq!(Some((0, 0b00000001)), bit_by_index(7, &[0b1000_0001]));
        assert_eq!(None, bit_by_index(6, &[0b1000_0001]));
        assert_eq!(None, bit_by_index(8, &[0b1000_0001]));
        assert_eq!(
            Some((1, 0b10000000)),
            bit_by_index(8, &[0b1000_0001, 0b1000_0000])
        );
    }
}
