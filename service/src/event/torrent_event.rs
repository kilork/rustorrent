use crate::{
    announce::Announcement,
    event::TorrentEventQueryPiece,
    file_download::FileDownloadStream,
    request_response::RequestResponse,
    result::RsbtResult,
    types::public::{AnnounceView, FileView, PeerView, TorrentDownloadState},
};
use std::{
    fmt::{Display, Formatter},
    ops::Range,
};
use tokio::{net::TcpStream, sync::watch};
use uuid::Uuid;

#[derive(Debug)]
pub(crate) enum TorrentEvent {
    Announce(Announcement),
    PeerConnected(Uuid, TcpStream),
    PeerForwarded(TcpStream),
    PeerConnectFailed(Uuid),
    PeerDisconnect(Uuid),
    PeerPieces(Uuid, Vec<u8>),
    PeerPiece(Uuid, usize),
    PeerUnchoke(Uuid),
    PeerInterested(Uuid),
    PeerPieceDownloaded(Uuid, Vec<u8>),
    PeerPieceCanceled(Uuid),
    PeerPieceRequest {
        peer_id: Uuid,
        index: u32,
        begin: u32,
        length: u32,
    },
    Enable(RequestResponse<(), RsbtResult<()>>),
    Disable(RequestResponse<(), RsbtResult<()>>),
    Subscribe(RequestResponse<(), watch::Receiver<TorrentDownloadState>>),
    Delete(RequestResponse<bool, RsbtResult<()>>),
    PeersView(RequestResponse<(), RsbtResult<Vec<PeerView>>>),
    AnnounceView(RequestResponse<(), RsbtResult<Vec<AnnounceView>>>),
    FilesView(RequestResponse<(), RsbtResult<Vec<FileView>>>),
    FileDownload(RequestResponse<(usize, Option<Range<usize>>), RsbtResult<FileDownloadStream>>),
    QueryPiece(RequestResponse<TorrentEventQueryPiece, RsbtResult<Vec<u8>>>),
}

impl Display for TorrentEvent {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            TorrentEvent::PeerPieceDownloaded(uuid, data) => {
                write!(f, "PeerPieceDownloaded({}, [{}])", uuid, data.len())
            }
            _ => write!(f, "{:?}", self),
        }
    }
}
