use super::*;
use crate::types::Properties;
use app::{
    download_torrent::{DownloadTorrentEvent, DownloadTorrentEventQueryPiece},
    RequestResponse, RsbtFileView, TorrentProcess,
};
use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use failure::ResultExt;
use flat_storage::FlatStorage;
use flat_storage_mmap::{FileInfo, MmapFlatStorage};
use futures::future::BoxFuture;
use std::{
    io::Read,
    ops::Range,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread,
};
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
    FileInfo {
        file_id: usize,
        sender: oneshot::Sender<Result<FileInfo, RsbtError>>,
    },
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
enum RsbtFileDownloadState {
    Idle,
    SendQueryPiece(
        BoxFuture<'static, Result<(), RsbtError>>,
        Option<oneshot::Receiver<Result<Vec<u8>, RsbtError>>>,
    ),
    ReceiveQueryPiece(oneshot::Receiver<Result<Vec<u8>, RsbtError>>),
}

impl std::fmt::Debug for RsbtFileDownloadState {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "")
    }
}

#[derive(Debug)]
pub struct RsbtFileDownloadStream {
    pub name: String,
    pub file_size: usize,
    pub size: usize,
    pub left: usize,
    pub piece: usize,
    pub piece_offset: usize,
    pub range: Option<Range<usize>>,
    torrent_process: Arc<TorrentProcess>,
    state: RsbtFileDownloadState,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl Stream for RsbtFileDownloadStream {
    type Item = Result<Bytes, RsbtError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.left == 0 {
            self.get_mut().left = 0;
            Poll::Ready(None)
        } else {
            debug!("going to poll stream");
            let mut that = self.as_mut();
            {
                let mut waker = that.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());
            }
            debug!("starting loop");

            loop {
                match &mut that.state {
                    RsbtFileDownloadState::Idle => {
                        debug!("idle state: send message");
                        let (request_response, receiver) =
                            RequestResponse::new(DownloadTorrentEventQueryPiece {
                                piece: that.piece,
                                waker: that.waker.clone(),
                            });

                        let torrent_process = that.torrent_process.clone();
                        let future = async move {
                            torrent_process
                                .broker_sender
                                .clone()
                                .send(DownloadTorrentEvent::QueryPiece(request_response))
                                .map_err(RsbtError::from)
                                .await
                        };
                        let sender = future.boxed();

                        that.state = RsbtFileDownloadState::SendQueryPiece(sender, Some(receiver));
                    }
                    RsbtFileDownloadState::SendQueryPiece(ref mut sender, receiver) => {
                        debug!("send query state: poll");
                        match sender.as_mut().poll(cx) {
                            Poll::Ready(Ok(())) => {
                                debug!("send query piece: ok");
                                that.state = RsbtFileDownloadState::ReceiveQueryPiece(
                                    receiver.take().unwrap(),
                                )
                            }
                            Poll::Ready(Err(err)) => {
                                error!("send query piece: err: {}", err);
                                return Poll::Ready(Some(Err(err)));
                            }
                            Poll::Pending => {
                                debug!("send query piece: pending");
                                return Poll::Pending;
                            }
                        }
                    }
                    RsbtFileDownloadState::ReceiveQueryPiece(receiver) => {
                        debug!("receive query state: poll");
                        match receiver.poll_unpin(cx) {
                            Poll::Ready(Ok(Ok(data))) => {
                                debug!("receive query piece: received data");
                                let remains = data.len() - that.piece_offset;
                                let size = if remains < that.left {
                                    remains
                                } else {
                                    that.left
                                };
                                let out = &data[that.piece_offset..that.piece_offset + size];
                                that.left -= size;
                                that.piece += 1;
                                that.piece_offset = 0;
                                that.state = RsbtFileDownloadState::Idle;
                                debug!("receive query piece: return data {}", out.len());
                                return Poll::Ready(Some(Ok(Bytes::from(out.to_owned()))));
                            }
                            Poll::Ready(Ok(Err(err))) => {
                                error!("receive query piece: received err: {}", err);
                                return Poll::Ready(Some(Err(err)));
                            }
                            Poll::Ready(Err(err)) => {
                                error!("receive query piece: received send err: {}", err);
                                return Poll::Ready(Some(Err(err.into())));
                            }
                            Poll::Pending => {
                                debug!("receive query piece: pending");
                                return Poll::Pending;
                            }
                        }
                    }
                }
            }
        }
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

impl TorrentStorage {
    pub async fn new<P: AsRef<Path>>(
        properties: Arc<Properties>,
        torrent_name: P,
        torrent_process: Arc<TorrentProcess>,
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

    pub async fn files(&self) -> Result<Vec<RsbtFileView>, RsbtError> {
        self.message(TorrentStorageMessage::Files).await
    }

    pub async fn download(
        &self,
        file_id: usize,
        range: Option<Range<usize>>,
    ) -> Result<RsbtFileDownloadStream, RsbtError> {
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
        Ok(RsbtFileDownloadStream {
            name: file_info.file.path.to_string_lossy().into(),
            file_size,
            size,
            left: size,
            piece,
            piece_offset,
            range,
            state: RsbtFileDownloadState::Idle,
            torrent_process: self.torrent_process.clone(),
            waker: Arc::new(Mutex::new(None)),
        })
    }
}

fn torrent_storage_message_loop(
    properties: Arc<Properties>,
    torrent_process: Arc<TorrentProcess>,
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

#[derive(Debug, Clone)]
pub struct TorrentPiece(Vec<u8>);

impl Deref for TorrentPiece {
    type Target = dyn AsRef<[u8]>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
