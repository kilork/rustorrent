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

/// Flat storage of different sized files.
///
/// Access to data is asynchronious. Files created lazy.
pub trait FlatStorage<F: Future<Output = Result<(), FlatStorageError>>> {
    fn files(&self) -> &[FlatStorageFile];
    fn read_block(&self, begin: usize, block: &mut [u8]) -> F;
    fn write_block(&self, begin: usize, block: &[u8]) -> F;
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
