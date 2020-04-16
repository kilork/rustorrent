use crate::{storage::TORRENT_STORAGE_FORMAT_VERSION, RsbtError};
use byteorder::{BigEndian, ReadBytesExt};
use failure::ResultExt;
use std::{convert::TryInto, io::Read, path::Path};
use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct TorrentStorageState {
    pub downloaded: Vec<u8>,
    pub bytes_write: u64,
    pub bytes_read: u64,
    pub pieces_left: u32,
}

impl TorrentStorageState {
    pub(crate) fn from_reader(mut rdr: impl Read) -> Result<Self, RsbtError> {
        let version = rdr.read_u8()?;
        if version != TORRENT_STORAGE_FORMAT_VERSION {
            return Err(RsbtError::StorageVersion(version));
        }
        let bytes_write: u64 = rdr.read_u64::<BigEndian>()?;
        let bytes_read = rdr.read_u64::<BigEndian>()?;
        let pieces_left = rdr.read_u32::<BigEndian>()?;
        let mut downloaded = vec![];
        rdr.read_to_end(&mut downloaded)?;
        Ok(Self {
            downloaded,
            bytes_write,
            bytes_read,
            pieces_left,
        })
    }

    pub(crate) async fn write_to_file(&self, mut f: File) -> Result<(), RsbtError> {
        f.write_u8(TORRENT_STORAGE_FORMAT_VERSION).await?;
        f.write_u64(self.bytes_write.try_into()?).await?;
        f.write_u64(self.bytes_read.try_into()?).await?;
        f.write_u32(self.pieces_left.try_into()?).await?;
        f.write_all(&self.downloaded).await?;
        Ok(())
    }

    pub(crate) async fn save<P: AsRef<Path>>(&self, state_file: P) -> Result<(), RsbtError> {
        let state_file_ref = &state_file.as_ref();
        let f = File::create(&state_file).await.with_context(|err| {
            format!(
                "cannot create state file {}: {}",
                state_file_ref.to_string_lossy(),
                err
            )
        })?;
        self.write_to_file(f)
            .await
            .with_context(|err| {
                format!(
                    "cannot write state file {}: {}",
                    state_file.as_ref().to_string_lossy(),
                    err
                )
            })
            .map_err(|x| x.into())
    }
}
