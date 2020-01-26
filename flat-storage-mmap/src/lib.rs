use flat_storage::*;

pub struct MmapFlatStorage {
    files: Vec<FlatStorageFile>,
    piece_size: usize,
    pieces: Vec<MmapFlatStoragePiece>,
}

pub struct MmapFlatStoragePiece {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
