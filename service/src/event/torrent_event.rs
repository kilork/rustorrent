use crate::{
    event::TorrentEventQueryPiece,
    file_download::FileDownloadStream,
    request_response::RequestResponse,
    types::{
        public::{AnnounceView, FileView, PeerView, TorrentDownloadState},
        Peer,
    },
    RsbtError,
};
use std::{
    fmt::{Display, Formatter},
    ops::Range,
};
use tokio::{net::TcpStream, sync::watch};
use uuid::Uuid;

#[derive(Debug)]
pub(crate) enum TorrentEvent {
    Announce(Vec<Peer>),
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
    Enable(RequestResponse<(), Result<(), RsbtError>>),
    Disable(RequestResponse<(), Result<(), RsbtError>>),
    Subscribe(RequestResponse<(), watch::Receiver<TorrentDownloadState>>),
    Delete(RequestResponse<bool, Result<(), RsbtError>>),
    PeersView(RequestResponse<(), Result<Vec<PeerView>, RsbtError>>),
    AnnounceView(RequestResponse<(), Result<Vec<AnnounceView>, RsbtError>>),
    FilesView(RequestResponse<(), Result<Vec<FileView>, RsbtError>>),
    FileDownload(
        RequestResponse<(usize, Option<Range<usize>>), Result<FileDownloadStream, RsbtError>>,
    ),
    QueryPiece(RequestResponse<TorrentEventQueryPiece, Result<Vec<u8>, RsbtError>>),
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
