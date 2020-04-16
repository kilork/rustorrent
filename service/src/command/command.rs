use crate::{
    command::{
        CommandAddTorrent, CommandDeleteTorrent, CommandTorrentAction, CommandTorrentAnnounce,
        CommandTorrentDetail, CommandTorrentFileDownload, CommandTorrentFiles, CommandTorrentPeers,
        CommandTorrentPieces,
    },
    file_download::FileDownloadStream,
    process::{TorrentProcess, TorrentToken},
    request_response::RequestResponse,
    types::{
        public::{AnnounceView, FileView, PeerView, TorrentDownloadView},
        Handshake,
    },
    RsbtError,
};
use std::sync::Arc;
use tokio::sync::oneshot;

#[derive(Debug)]
pub enum Command {
    AddTorrent(RequestResponse<CommandAddTorrent, Result<TorrentProcess, RsbtError>>),
    DeleteTorrent(RequestResponse<CommandDeleteTorrent, Result<(), RsbtError>>),
    TorrentHandshake {
        handshake_request: Handshake,
        handshake_sender: oneshot::Sender<Option<Arc<TorrentToken>>>,
    },
    TorrentList(RequestResponse<(), Result<Vec<TorrentDownloadView>, RsbtError>>),
    TorrentAction(RequestResponse<CommandTorrentAction, Result<(), RsbtError>>),
    TorrentPeers(RequestResponse<CommandTorrentPeers, Result<Vec<PeerView>, RsbtError>>),
    TorrentDetail(RequestResponse<CommandTorrentDetail, Result<TorrentDownloadView, RsbtError>>),
    TorrentAnnounces(RequestResponse<CommandTorrentAnnounce, Result<Vec<AnnounceView>, RsbtError>>),
    TorrentFiles(RequestResponse<CommandTorrentFiles, Result<Vec<FileView>, RsbtError>>),
    TorrentPieces(RequestResponse<CommandTorrentPieces, Result<Vec<u8>, RsbtError>>),
    TorrentFileDownloadHeader(
        RequestResponse<CommandTorrentFileDownload, Result<FileView, RsbtError>>,
    ),
    TorrentFileDownload(
        RequestResponse<CommandTorrentFileDownload, Result<FileDownloadStream, RsbtError>>,
    ),
}
