use flat_storage::*;
use log::debug;
use memmap::MmapMut;
use std::{
    fs::{create_dir_all, remove_file, OpenOptions},
    path::Path,
    sync::Mutex,
};

pub struct MmapFlatStorage {
    files: Vec<FlatStorageFile>,
    file_handles: Vec<Mutex<FileHandle>>,
    mapping: Vec<MmapFlatStorageMapping>,
}

#[derive(Debug, PartialEq)]
struct MmapFlatStorageMapping(Vec<FileBlock>);

#[derive(Debug, PartialEq)]
struct FileBlock {
    offset: usize,
    file_index: usize,
    file_offset: usize,
    size: usize,
}

struct FileHandle {
    mmap: Option<MmapMut>,
    saved: usize,
}

impl MmapFlatStorage {
    pub fn create<P: AsRef<Path>>(
        download_path: P,
        piece_size: usize,
        files: Vec<FlatStorageFile>,
        downloaded: &[u8],
    ) -> Result<Self, std::io::Error> {
        let mapping = map_pieces_to_files(piece_size, &files);
        let file_handles = load_files(&download_path, &files, downloaded, &mapping)?;
        Ok(Self {
            files,
            file_handles,
            mapping,
        })
    }

    pub fn delete_files<P: AsRef<Path>>(&self, download_path: P) -> Result<(), std::io::Error> {
        for file_handle in &self.file_handles {
            if let Some(mut file_handle) = file_handle.lock().ok() {
                if let Some(mmap) = file_handle.mmap.take() {
                    mmap.flush()?;
                }
            }
        }
        for file in &self.files {
            let file_path = download_path.as_ref().join(&file.path);
            debug!("deleting file: {:?}", file_path);
            if file_path.is_file() {
                remove_file(file_path)?
            }
        }
        Ok(())
    }

    pub fn saved(&self) -> Vec<usize> {
        self.file_handles
            .iter()
            .map(|x| x.lock().unwrap().saved)
            .collect()
    }
}

fn load_files<P: AsRef<Path>>(
    download_path: P,
    files: &[FlatStorageFile],
    downloaded: &[u8],
    mapping: &[MmapFlatStorageMapping],
) -> Result<Vec<Mutex<FileHandle>>, std::io::Error> {
    let mut result = vec![];
    for (index, file) in files.iter().enumerate() {
        let saved = saved(index, mapping, downloaded);
        let file_path = download_path.as_ref().join(&file.path);
        debug!("checking file: {:?}", file_path);
        if !file_path.is_file() {
            if let Some(path) = file_path.parent() {
                debug!("create dir {:?}", path);
                create_dir_all(path)?;
            }
        }
        debug!("create file");
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&file_path)?;
        debug!("set len");
        f.set_len(file.length as u64)?;
        debug!("creating mmap...");
        let mmap = Some(unsafe { MmapMut::map_mut(&f)? });
        result.push(Mutex::new(FileHandle { mmap, saved }));
        debug!("processed file: {:?}", file_path);
    }
    Ok(result)
}

impl FlatStorage for MmapFlatStorage {
    fn files(&self) -> &[FlatStorageFile] {
        &self.files
    }

    fn read_piece<I: Into<FlatStoragePieceIndex>>(
        &self,
        index: I,
    ) -> Result<Option<Vec<u8>>, FlatStorageError> {
        let map_to_files = &self.mapping[*index.into()];
        let mut result = vec![];
        for file_block in &map_to_files.0 {
            let f = &self.file_handles[file_block.file_index];
            if let Some(data) = &f.lock().unwrap().mmap {
                let data = &data[file_block.file_offset..file_block.file_offset + file_block.size];
                result.extend_from_slice(data);
            }
        }

        Ok(Some(result))
    }

    fn write_piece<I: Into<FlatStoragePieceIndex>>(
        &self,
        index: I,
        block: Vec<u8>,
    ) -> Result<(), FlatStorageError> {
        let map_to_files = &self.mapping[*index.into()];
        for file_block in &map_to_files.0 {
            let f = &self.file_handles[file_block.file_index];
            let mut f_lock = f.lock().unwrap();
            f_lock.saved += file_block.size;
            if let Some(data) = f_lock.mmap.as_mut() {
                let data =
                    &mut data[file_block.file_offset..file_block.file_offset + file_block.size];
                data.copy_from_slice(&block[file_block.offset..file_block.offset + file_block.size])
            }
        }

        Ok(())
    }
}

fn map_pieces_to_files(
    piece_size: usize,
    files: &[FlatStorageFile],
) -> Vec<MmapFlatStorageMapping> {
    let mut current_piece_left = piece_size;
    let mut current_piece = MmapFlatStorageMapping(vec![]);
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
            current_piece_left = piece_size;

            mapping.push(current_piece);
            current_piece = MmapFlatStorageMapping(vec![]);
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

fn saved(file_index: usize, mapping: &[MmapFlatStorageMapping], downloaded: &[u8]) -> usize {
    let mut saved = 0;
    for (downloaded_block_index, downloaded_block) in downloaded.iter().enumerate() {
        for subindex in 0..8 {
            if downloaded_block & 1 << subindex != 0 {
                let index = downloaded_block_index * 8 + subindex;
                let mapping_block = &mapping[index];
                for file_block in &mapping_block.0 {
                    if file_block.file_index == file_index {
                        saved += file_block.size;
                    }
                }
            }
        }
    }
    saved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_to_files() {
        let result = map_pieces_to_files(
            100,
            &[FlatStorageFile {
                path: "test".into(),
                length: 1000,
            }],
        );
        dbg!(&result);
        assert_eq!(result.len(), 10);

        let result = map_pieces_to_files(
            1000,
            &[FlatStorageFile {
                path: "test".into(),
                length: 1000,
            }],
        );
        assert_eq!(
            result,
            vec![MmapFlatStorageMapping(vec![FileBlock {
                offset: 0,
                file_index: 0,
                file_offset: 0,
                size: 1000,
            }])]
        );

        let result = map_pieces_to_files(
            1000,
            &[FlatStorageFile {
                path: "test".into(),
                length: 800,
            }],
        );
        assert_eq!(
            result,
            vec![MmapFlatStorageMapping(vec![FileBlock {
                offset: 0,
                file_index: 0,
                file_offset: 0,
                size: 800,
            }])]
        );

        let result = map_pieces_to_files(
            333,
            &[FlatStorageFile {
                path: "test".into(),
                length: 1000,
            }],
        );
        assert_eq!(
            result,
            vec![
                MmapFlatStorageMapping(vec![FileBlock {
                    offset: 0,
                    file_index: 0,
                    file_offset: 0,
                    size: 333,
                }]),
                MmapFlatStorageMapping(vec![FileBlock {
                    offset: 0,
                    file_index: 0,
                    file_offset: 333,
                    size: 333,
                }]),
                MmapFlatStorageMapping(vec![FileBlock {
                    offset: 0,
                    file_index: 0,
                    file_offset: 666,
                    size: 333,
                }]),
                MmapFlatStorageMapping(vec![FileBlock {
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
                FlatStorageFile {
                    path: "test1".into(),
                    length: 300,
                },
                FlatStorageFile {
                    path: "test2".into(),
                    length: 400,
                },
                FlatStorageFile {
                    path: "test3".into(),
                    length: 500,
                },
            ],
        );
        assert_eq!(
            result,
            vec![
                MmapFlatStorageMapping(vec![
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
                MmapFlatStorageMapping(vec![
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
                MmapFlatStorageMapping(vec![FileBlock {
                    offset: 0,
                    file_index: 2,
                    file_offset: 300,
                    size: 200,
                }])
            ]
        );
    }
}
