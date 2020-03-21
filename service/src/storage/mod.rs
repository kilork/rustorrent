use super::*;
use crate::types::Properties;
use app::TorrentProcess;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use failure::ResultExt;
use flat_storage::FlatStorage;
use flat_storage_mmap::MmapFlatStorage;
use std::{
    io::{Read, Write},
    thread,
};
use tokio::{
    fs::{self, File},
    runtime::Builder,
};

#[derive(Debug)]
pub struct TorrentStorage {
    pub handle: std::thread::JoinHandle<Result<(), RsbtError>>,
    torrent_process: Arc<TorrentProcess>,
    sender: Sender<TorrentStorageMessage>,
    pub receiver: tokio::sync::watch::Receiver<TorrentStorageState>,
}

enum TorrentStorageMessage {
    LoadPiece {
        index: usize,
        sender: tokio::sync::oneshot::Sender<Result<Option<TorrentPiece>, RsbtError>>,
    },
    SavePiece {
        index: usize,
        data: Vec<u8>,
        sender: tokio::sync::oneshot::Sender<Result<(), RsbtError>>,
    },
}

#[derive(Clone, Debug)]
pub struct TorrentStorageState {
    pub downloaded: Vec<u8>,
    pub bytes_downloaded: u64,
    pub bytes_uploaded: u64,
    pub pieces_left: u32,
}

impl TorrentStorageState {
    fn from_reader(mut rdr: impl Read) -> Result<Self, RsbtError> {
        let _version = rdr.read_u8()?;
        let bytes_downloaded: u64 = rdr.read_u64::<BigEndian>()?;
        let bytes_uploaded = rdr.read_u64::<BigEndian>()?;
        let pieces_left = rdr.read_u32::<BigEndian>()?;
        let mut downloaded = vec![];
        rdr.read_to_end(&mut downloaded)?;
        Ok(Self {
            downloaded,
            bytes_downloaded: bytes_downloaded,
            bytes_uploaded: bytes_uploaded,
            pieces_left: pieces_left,
        })
    }

    async fn write_to_file(&self, mut f: File) -> Result<(), RsbtError> {
        f.write_u8(0).await?;
        f.write_u64(self.bytes_downloaded.try_into()?).await?;
        f.write_u64(self.bytes_uploaded.try_into()?).await?;
        f.write_u32(self.pieces_left.try_into()?).await?;
        f.write_all(&self.downloaded).await?;
        Ok(())
    }

    async fn save<P: AsRef<Path>>(&self, state_file: P) -> Result<(), RsbtError> {
        let f = File::create(state_file).await?;
        self.write_to_file(f).await
    }
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
        let data = fs::read(&torrent_storage_state_file).await?;
        TorrentStorageState::from_reader(data.as_slice())?
    } else {
        let state = TorrentStorageState {
            downloaded: vec![],
            bytes_downloaded: 0,
            bytes_uploaded: 0,
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
        let (state_file, mut state) =
            prepare_storage_state(properties.clone(), torrent_name, torrent_process.clone())
                .await?;
        let (sender, mut channel_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);
        let mut pieces_left = torrent_process.info.pieces.len();

        let (watch_sender, receiver) = tokio::sync::watch::channel(state.clone());

        let thread_torrent_process = torrent_process.clone();

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

                            match tokio::task::spawn_blocking(move || {
                                storage.write_piece(index, data)
                            })
                            .await?
                            {
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
                                pieces_left -= 1;
                                state.bytes_downloaded += len as u64;
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

                            let piece = match tokio::task::spawn_blocking(move || {
                                storage.read_piece(index)
                            })
                            .await?
                            {
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
                                state.bytes_uploaded += piece.as_ref().len() as u64;
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

    pub async fn save(&mut self, index: usize, data: Vec<u8>) -> Result<(), RsbtError> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.sender
            .send(TorrentStorageMessage::SavePiece {
                index,
                data,
                sender,
            })
            .await?;

        receiver.await?
    }

    pub async fn load(&mut self, index: usize) -> Result<Option<TorrentPiece>, RsbtError> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.sender
            .send(TorrentStorageMessage::LoadPiece { index, sender })
            .await?;

        receiver.await?
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
