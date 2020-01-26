use super::*;
use crate::types::Settings;
use app::TorrentProcess;
use flat_storage::FlatStorage;
use flat_storage_mmap::MmapFlatStorage;
use std::{ops::Range, thread};
use tokio::runtime::Builder;

#[derive(Debug)]
pub struct TorrentStorage {
    pub handle: std::thread::JoinHandle<Result<(), RustorrentError>>,
    torrent_process: Arc<TorrentProcess>,
    sender: Sender<TorrentStorageMessage>,
    pub receiver: tokio::sync::watch::Receiver<TorrentStorageState>,
}

enum TorrentStorageMessage {
    LoadPiece {
        index: usize,
        sender: tokio::sync::oneshot::Sender<Option<TorrentPiece>>,
    },
    SavePiece {
        index: usize,
        data: Vec<u8>,
        sender: tokio::sync::oneshot::Sender<()>,
    },
}

#[derive(Clone, Debug)]
pub struct TorrentStorageState {
    pub downloaded: Vec<u8>,
    pub pieces_left: usize,
}

impl TorrentStorage {
    pub fn new(settings: Arc<Settings>, torrent_process: Arc<TorrentProcess>) -> Self {
        let (sender, mut channel_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);
        let mut pieces_left = torrent_process.info.pieces.len();

        let (watch_sender, receiver) = tokio::sync::watch::channel(TorrentStorageState {
            downloaded: vec![],
            pieces_left,
        });

        let thread_torrent_process = torrent_process.clone();

        let handle = thread::spawn(move || {
            let info = &thread_torrent_process.info;
            let mut rt = Builder::new().basic_scheduler().enable_io().build()?;
            let mut downloaded = vec![];
            let mmap_storage = Arc::new(MmapFlatStorage::create(
                ".",
                info.piece_length,
                info.files.clone(),
                &downloaded,
            )?);
            rt.block_on(async move {
                let mut bytes_downloaded = 0;
                // let mut bytes_uploaded = 0;

                while let Some(message) = channel_receiver.next().await {
                    match message {
                        TorrentStorageMessage::SavePiece {
                            index,
                            data,
                            sender,
                        } => {
                            let (block_index, bit) = crate::messages::index_in_bitarray(index);
                            while downloaded.len() <= index {
                                downloaded.push(0);
                            }
                            if downloaded[block_index] & bit == 0 {
                                pieces_left -= 1;
                                bytes_downloaded += data.len();
                            }
                            downloaded[block_index] |= bit;

                            if let Err(err) = watch_sender.broadcast(TorrentStorageState {
                                downloaded: downloaded.clone(),
                                pieces_left,
                            }) {
                                error!("cannot notify watchers: {}", err);
                            }

                            if let Err(_) = sender.send(()) {
                                error!("cannot send oneshot");
                            }
                            let storage = mmap_storage.clone();

                            tokio::task::spawn_blocking(move || storage.write_piece(index, data))
                                .await??;
                        }
                        TorrentStorageMessage::LoadPiece { index, sender } => {
                            let storage = mmap_storage.clone();

                            let piece =
                                tokio::task::spawn_blocking(move || storage.read_piece(index))
                                    .await??
                                    .map(|x| TorrentPiece(x));

                            if let Err(_) = sender.send(piece) {
                                error!("cannot send piece with oneshot message");
                            }
                        }
                    }
                }
                Ok::<(), RustorrentError>(())
            })?;

            Ok(())
        });

        Self {
            handle,
            torrent_process,
            sender,
            receiver,
        }
    }

    pub async fn save(&mut self, index: usize, data: Vec<u8>) -> Result<(), RustorrentError> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.sender
            .send(TorrentStorageMessage::SavePiece {
                index,
                data,
                sender,
            })
            .await?;

        receiver.map_err(|x| x.into()).await
    }

    pub async fn load(&mut self, index: usize) -> Result<Option<TorrentPiece>, RustorrentError> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.sender
            .send(TorrentStorageMessage::LoadPiece { index, sender })
            .await?;

        receiver.map_err(|x| x.into()).await
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
