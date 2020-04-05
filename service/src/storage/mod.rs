use super::*;
use crate::types::Properties;
use app::{RsbtFileView, TorrentProcess};
use byteorder::{BigEndian, ReadBytesExt};
use failure::ResultExt;
use flat_storage::FlatStorage;
use flat_storage_mmap::MmapFlatStorage;
use std::{io::Read, thread};
use tokio::{
    fs::{self, File},
    runtime::Builder,
    sync::{oneshot, watch},
    task::spawn_blocking,
};

#[derive(Debug)]
pub struct TorrentStorage {
    pub handle: std::thread::JoinHandle<Result<(), RsbtError>>,
    torrent_process: Arc<TorrentProcess>,
    sender: Sender<TorrentStorageMessage>,
    pub receiver: watch::Receiver<TorrentStorageState>,
}

enum TorrentStorageMessage {
    LoadPiece {
        index: usize,
        sender: oneshot::Sender<Result<Option<TorrentPiece>, RsbtError>>,
    },
    SavePiece {
        index: usize,
        data: Vec<u8>,
        sender: oneshot::Sender<Result<(), RsbtError>>,
    },
    Delete {
        files: bool,
        sender: oneshot::Sender<Result<(), RsbtError>>,
    },
    Files(oneshot::Sender<Result<Vec<RsbtFileView>, RsbtError>>),
}

#[derive(Clone, Debug)]
pub struct TorrentStorageState {
    pub downloaded: Vec<u8>,
    pub bytes_write: u64,
    pub bytes_read: u64,
    pub pieces_left: u32,
}

const TORRENT_STORAGE_FORMAT_VERSION: u8 = 0;

impl TorrentStorageState {
    fn from_reader(mut rdr: impl Read) -> Result<Self, RsbtError> {
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

    async fn write_to_file(&self, mut f: File) -> Result<(), RsbtError> {
        f.write_u8(TORRENT_STORAGE_FORMAT_VERSION).await?;
        f.write_u64(self.bytes_write.try_into()?).await?;
        f.write_u64(self.bytes_read.try_into()?).await?;
        f.write_u32(self.pieces_left.try_into()?).await?;
        f.write_all(&self.downloaded).await?;
        Ok(())
    }

    async fn save<P: AsRef<Path>>(&self, state_file: P) -> Result<(), RsbtError> {
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

async fn cleanup_storage_state<P: AsRef<Path>>(
    properties: Arc<Properties>,
    torrent_name: P,
) -> Result<(), RsbtError> {
    let storage_torrent_file = properties.storage.join(torrent_name.as_ref());

    if storage_torrent_file.is_file() {
        fs::remove_file(&storage_torrent_file).await?;
    }

    let mut torrent_storage_state_file = storage_torrent_file.clone();
    torrent_storage_state_file.set_extension("torrent.state");

    if torrent_storage_state_file.is_file() {
        fs::remove_file(&torrent_storage_state_file).await?;
    }

    Ok(())
}

async fn prepare_storage_state<P: AsRef<Path>>(
    properties: Arc<Properties>,
    torrent_name: P,
    torrent_process: Arc<TorrentProcess>,
) -> Result<(PathBuf, TorrentStorageState), RsbtError> {
    let storage_torrent_file = properties.storage.join(torrent_name.as_ref());

    if !storage_torrent_file.is_file() {
        fs::write(&storage_torrent_file, &torrent_process.torrent.raw)
            .await
            .with_context(|err| {
                format!(
                    "cannot save torrent file {}: {}",
                    storage_torrent_file.to_string_lossy(),
                    err
                )
            })?;
    }

    let mut torrent_storage_state_file = storage_torrent_file.clone();
    torrent_storage_state_file.set_extension("torrent.state");

    let torrent_storage_state = if torrent_storage_state_file.is_file() {
        let data = fs::read(&torrent_storage_state_file)
            .await
            .with_context(|err| {
                format!(
                    "cannot read torrent state file {}: {}",
                    storage_torrent_file.to_string_lossy(),
                    err
                )
            })?;
        TorrentStorageState::from_reader(data.as_slice())?
    } else {
        let state = TorrentStorageState {
            downloaded: vec![],
            bytes_write: 0,
            bytes_read: 0,
            pieces_left: torrent_process.info.pieces.len() as u32,
        };
        state.save(&torrent_storage_state_file).await?;
        state
    };
    Ok((torrent_storage_state_file, torrent_storage_state))
}

impl TorrentStorage {
    pub async fn new<P: AsRef<Path>>(
        properties: Arc<Properties>,
        torrent_name: P,
        torrent_process: Arc<TorrentProcess>,
    ) -> Result<Self, RsbtError> {
        let (state_file, mut state) = prepare_storage_state(
            properties.clone(),
            torrent_name.as_ref(),
            torrent_process.clone(),
        )
        .await?;
        let (sender, mut channel_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let (watch_sender, receiver) = watch::channel(state.clone());

        let thread_torrent_process = torrent_process.clone();

        let thread_torrent_name = PathBuf::from(torrent_name.as_ref());

        let handle = thread::spawn(move || {
            let info = &thread_torrent_process.info;
            let mut rt = Builder::new().basic_scheduler().enable_io().build()?;
            let mmap_storage = Arc::new(MmapFlatStorage::create(
                properties.save_to.clone(),
                info.piece_length,
                info.files.clone(),
                &state.downloaded,
            )?);
            rt.block_on(async move {
                while let Some(message) = channel_receiver.next().await {
                    match message {
                        TorrentStorageMessage::SavePiece {
                            index,
                            data,
                            sender,
                        } => {
                            let (block_index, bit) = crate::messages::index_in_bitarray(index);

                            let storage = mmap_storage.clone();
                            let len = data.len();

                            match spawn_blocking(move || storage.write_piece(index, data)).await? {
                                Ok(()) => (),
                                Err(err) => {
                                    error!("cannot write piece: {}", err);
                                    if sender.send(Err(err.into())).is_err() {
                                        error!("cannot send piece with oneshot message");
                                    }
                                    continue;
                                }
                            }

                            while state.downloaded.len() <= index {
                                state.downloaded.push(0);
                            }
                            if state.downloaded[block_index] & bit == 0 {
                                state.pieces_left -= 1;
                                state.bytes_write += len as u64;
                            }
                            state.downloaded[block_index] |= bit;

                            if let Err(err) = state.save(&state_file).await {
                                error!("cannot save state: {}", err);
                            }
                            if let Err(err) = watch_sender.broadcast(state.clone()) {
                                error!("cannot notify watchers: {}", err);
                            }

                            if sender.send(Ok(())).is_err() {
                                error!("cannot send oneshot");
                            }
                        }
                        TorrentStorageMessage::LoadPiece { index, sender } => {
                            let storage = mmap_storage.clone();

                            let piece =
                                match spawn_blocking(move || storage.read_piece(index)).await? {
                                    Ok(data) => data.map(TorrentPiece),
                                    Err(err) => {
                                        error!("cannot read piece: {}", err);
                                        if sender.send(Err(err.into())).is_err() {
                                            error!("cannot send piece with oneshot message");
                                        }
                                        continue;
                                    }
                                };

                            if let Some(piece) = &piece {
                                state.bytes_read += piece.as_ref().len() as u64;
                            }

                            if let Err(err) = state.save(&state_file).await {
                                error!("cannot save state: {}", err);
                            }
                            if let Err(err) = watch_sender.broadcast(state.clone()) {
                                error!("cannot notify watchers: {}", err);
                            }

                            if sender.send(Ok(piece)).is_err() {
                                error!("cannot send piece with oneshot message");
                            }
                        }
                        TorrentStorageMessage::Delete { files, sender } => {
                            let mut result =
                                cleanup_storage_state(properties.clone(), thread_torrent_name)
                                    .await;
                            if files {
                                let storage = mmap_storage.clone();
                                result = spawn_blocking(move || {
                                    storage
                                        .delete_files(properties.save_to.clone())
                                        .map_err(RsbtError::from)
                                })
                                .await?;
                            }
                            if sender.send(result).is_err() {
                                error!("cannot send delete result with oneshot message");
                            }
                            break;
                        }
                        TorrentStorageMessage::Files(sender) => {
                            let storage = mmap_storage.clone();
                            let saved = spawn_blocking(move || storage.saved())
                                .await
                                .map_err(RsbtError::from);
                            let files_view = saved.map(|saved| {
                                saved
                                    .into_iter()
                                    .zip(info.files.iter())
                                    .enumerate()
                                    .map(|(id, (saved, info))| RsbtFileView {
                                        id,
                                        name: info.path.to_string_lossy().into(),
                                        saved,
                                        size: info.length,
                                    })
                                    .collect()
                            });
                            if sender.send(files_view).is_err() {
                                error!("cannot send files result with oneshot message");
                            }
                        }
                    }
                }
                Ok::<(), RsbtError>(())
            })?;

            Ok(())
        });

        Ok(Self {
            handle,
            torrent_process,
            sender,
            receiver,
        })
    }

    async fn message<R, F>(&self, f: F) -> Result<R, RsbtError>
    where
        F: FnOnce(oneshot::Sender<Result<R, RsbtError>>) -> TorrentStorageMessage,
    {
        let (sender, receiver) = oneshot::channel();

        self.sender.clone().send(f(sender)).await?;

        receiver.await?
    }

    pub async fn save(&self, index: usize, data: Vec<u8>) -> Result<(), RsbtError> {
        self.message(|sender| TorrentStorageMessage::SavePiece {
            index,
            data,
            sender,
        })
        .await
    }

    pub async fn load(&self, index: usize) -> Result<Option<TorrentPiece>, RsbtError> {
        self.message(|sender| TorrentStorageMessage::LoadPiece { index, sender })
            .await
    }

    pub async fn delete(&self, files: bool) -> Result<(), RsbtError> {
        self.message(|sender| TorrentStorageMessage::Delete { files, sender })
            .await
    }

    pub async fn files(&self) -> Result<Vec<RsbtFileView>, RsbtError> {
        self.message(TorrentStorageMessage::Files).await
    }

    pub async fn download(
        &self,
        file_id: usize,
    ) -> Result<crate::app::RsbtFileDownloadStream, RsbtError> {
        Err(RsbtError::TorrentActionNotSupported)
    }
}

#[derive(Debug, Clone)]
pub struct TorrentPiece(Vec<u8>);

impl Deref for TorrentPiece {
    type Target = dyn AsRef<[u8]>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
