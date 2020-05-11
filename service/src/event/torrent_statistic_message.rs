use crate::{request_response::RequestResponse, types::public::TorrentDownloadState};
use tokio::sync::watch;

pub enum TorrentStatisticMessage {
    Subscribe(RequestResponse<(), watch::Receiver<TorrentDownloadState>>),
    Downloaded(u64),
    Uploaded(u64),
}
