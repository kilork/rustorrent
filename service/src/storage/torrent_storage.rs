use crate::process::TorrentToken;
use crate::{
    file_download::{FileDownloadState, FileDownloadStream},
    storage::{TorrentPiece, TorrentStorageMessage, TorrentStorageState},
    types::{public::FileView, Properties},
    RsbtError, DEFAULT_CHANNEL_BUFFER,
};
use failure::ResultExt;
use flat_storage::{index_in_bitarray, FlatStorage};
use flat_storage_mmap::MmapFlatStorage;
use futures::StreamExt;
use log::{debug, error};
use std::{
    ops::Range,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};
use tokio::{
    fs,
    runtime::Builder,
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot, watch,
    },
    task::spawn_blocking,
};

#[derive(Debug)]
pub struct TorrentStorage {
    pub handle: std::thread::JoinHandle<Result<(), RsbtError>>,
    torrent_process: Arc<TorrentToken>,
    sender: Sender<TorrentStorageMessage>,
    pub receiver: watch::Receiver<TorrentStorageState>,
}

impl TorrentStorage {
    pub async fn new<P: AsRef<Path>>(
        properties: Arc<Properties>,
        torrent_name: P,
        torrent_process: Arc<TorrentToken>,
    ) -> Result<Self, RsbtError> {
        let (state_file, state) = prepare_storage_state(
            properties.clone(),
            torrent_name.as_ref(),
            torrent_process.clone(),
        )
        .await?;
        let (sender, channel_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let (watch_sender, receiver) = watch::channel(state.clone());

        let thread_torrent_process = torrent_process.clone();

        let thread_torrent_name = PathBuf::from(torrent_name.as_ref());

        let handle = thread::spawn(move || {
            if let Err(err) = torrent_storage_message_loop(
                properties,
                thread_torrent_process,
                thread_torrent_name,
                state,
                state_file,
                channel_receiver,
                watch_sender,
            ) {
                error!("torrent storage loop failure: {}", err);
                Err(err)
            } else {
                Ok(())
            }
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

    pub async fn files(&self) -> Result<Vec<FileView>, RsbtError> {
        self.message(TorrentStorageMessage::Files).await
    }

    pub async fn download(
        &self,
        file_id: usize,
        range: Option<Range<usize>>,
    ) -> Result<FileDownloadStream, RsbtError> {
        let file_info = self
            .message(|sender| TorrentStorageMessage::FileInfo { file_id, sender })
            .await?;
        let file_size = file_info.file.length;
        let (size, piece, piece_offset) = if let Some(Range { start, end }) = range {
            if end > file_size {
                return Err(RsbtError::TorrentFileRangeInvalid { file_size });
            }
            let range_len = end - start;
            let piece_length = self.torrent_process.info.piece_length;
            let mut piece_offset = file_info.piece_offset + start;
            let piece = file_info.piece + piece_offset / piece_length;
            piece_offset %= piece_length;
            (range_len, piece, piece_offset)
        } else {
            (file_size, file_info.piece, file_info.piece_offset)
        };
        Ok(FileDownloadStream {
            name: file_info.file.path.to_string_lossy().into(),
            file_size,
            size,
            left: size,
            piece,
            piece_offset,
            range,
            state: FileDownloadState::Idle,
            torrent_process: self.torrent_process.clone(),
            waker: Arc::new(Mutex::new(None)),
        })
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
    torrent_process: Arc<TorrentToken>,
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
        debug!("loading state from: {:?}", torrent_storage_state_file);
        let data = fs::read(&torrent_storage_state_file)
            .await
            .with_context(|err| {
                format!(
                    "cannot read torrent state file {}: {}",
                    storage_torrent_file.to_string_lossy(),
                    err
                )
            })?;
        let state = TorrentStorageState::from_reader(data.as_slice())?;
        debug!("loaded state: {:?}", state);
        state
    } else {
        debug!("creating new state in: {:?}", torrent_storage_state_file);
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

fn torrent_storage_message_loop(
    properties: Arc<Properties>,
    torrent_process: Arc<TorrentToken>,
    torrent_name: PathBuf,
    mut state: TorrentStorageState,
    state_file: PathBuf,
    mut channel_receiver: Receiver<TorrentStorageMessage>,
    watch_sender: watch::Sender<TorrentStorageState>,
) -> Result<(), RsbtError> {
    let info = &torrent_process.info;
    let mut rt = Builder::new().basic_scheduler().enable_io().build()?;
    let mmap_storage = Arc::new(MmapFlatStorage::create(
        properties.save_to.clone(),
        info.pieces.len(),
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
                    let (block_index, bit) = index_in_bitarray(index);

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

                    while state.downloaded.len() <= block_index {
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

                    let piece = match spawn_blocking(move || storage.read_piece(index)).await? {
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
                    let mut result = cleanup_storage_state(properties.clone(), torrent_name).await;
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
                            .map(|(id, (saved, info))| FileView {
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
                TorrentStorageMessage::FileInfo { file_id, sender } => {
                    let storage = mmap_storage.clone();
                    let file_info = spawn_blocking(move || storage.file_info(file_id))
                        .await
                        .map_err(RsbtError::from)
                        .map_or_else(
                            |x| Err(x),
                            |v| v.ok_or_else(|| RsbtError::TorrentFileNotFound(file_id)),
                        );

                    if sender.send(file_info).is_err() {
                        error!("cannot send files result with oneshot message");
                    }
                }
            }
        }
        Ok::<(), RsbtError>(())
    })?;

    Ok(())
}
