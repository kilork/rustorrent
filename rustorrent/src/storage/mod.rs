use super::*;
use app::TorrentProcess;
use std::thread;
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
    pub fn new(torrent_process: Arc<TorrentProcess>) -> Self {
        let (sender, mut channel_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);
        let mut pieces_left = torrent_process.info.pieces.len();
        let (watch_sender, receiver) = tokio::sync::watch::channel(TorrentStorageState {
            downloaded: vec![],
            pieces_left,
        });

        let handle = thread::spawn(move || {
            let mut rt = Builder::new().basic_scheduler().enable_io().build()?;
            rt.block_on(async move {
                let mut pieces = vec![];
                let mut bytes_downloaded = 0;
                // let mut bytes_uploaded = 0;
                let mut downloaded = vec![];

                while let Some(message) = channel_receiver.next().await {
                    match message {
                        TorrentStorageMessage::SavePiece {
                            index,
                            data,
                            sender,
                        } => {
                            while pieces.len() <= index {
                                pieces.push(None);
                            }

                            if pieces[index].is_none() {
                                pieces_left -= 1;
                                bytes_downloaded += data.len();
                            }

                            pieces[index] = Some(TorrentPiece(data));

                            let (index, bit) = crate::messages::index_in_bitarray(index);
                            while downloaded.len() <= index {
                                downloaded.push(0);
                            }
                            downloaded[index] |= bit;

                            if let Err(err) = watch_sender.broadcast(TorrentStorageState {
                                downloaded: downloaded.clone(),
                                pieces_left,
                            }) {
                                error!("cannot notify watchers: {}", err);
                            }

                            if let Err(_) = sender.send(()) {
                                error!("cannot send oneshot");
                            }
                        }
                        TorrentStorageMessage::LoadPiece { index, sender } => {
                            if let Err(_) = sender.send(pieces.get(index).cloned().unwrap_or(None))
                            {
                                error!("cannot send oneshot");
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
