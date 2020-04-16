use crate::{storage::TorrentPiece, types::public::FileView, RsbtError};
use flat_storage_mmap::FileInfo;
use tokio::sync::oneshot;

pub(crate) enum TorrentStorageMessage {
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
    Files(oneshot::Sender<Result<Vec<FileView>, RsbtError>>),
    FileInfo {
        file_id: usize,
        sender: oneshot::Sender<Result<FileInfo, RsbtError>>,
    },
}
