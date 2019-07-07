use failure::Fail;
use futures::Future;
use std::path::Path;

/// Flat storage of different sized files.
///
/// Access to data is asynchronious. Files created lazy.
pub trait FlatStorage<F: Future<Item = (), Error = FlatStorageError>> {
    fn allocate_file<P: AsRef<Path>>(relative_path: P, file_size: usize);
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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
