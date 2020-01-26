use failure::Fail;
use std::{
    future::Future,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq)]
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

/// Flat storage of different sized files.
///
/// Access to data is asynchronious. Files created lazy.
pub trait FlatStorage {
    fn files(&self) -> &[FlatStorageFile];

    fn read_piece<
        I: Into<FlatStoragePieceIndex>,
        R: Future<Output = Result<Option<Vec<u8>>, FlatStorageError>>,
    >(
        &self,
        index: I,
    ) -> R;

    fn write_piece<
        I: Into<FlatStoragePieceIndex>,
        R: Future<Output = Result<(), FlatStorageError>>,
    >(
        &self,
        index: I,
        block: Vec<u8>,
    ) -> R;
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
