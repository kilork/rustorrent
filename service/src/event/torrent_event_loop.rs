use crate::{
    event::TorrentEvent, peer::PeerManager, process::TorrentToken, storage::TorrentStorage,
    types::Properties,
};
use futures::StreamExt;
use log::{debug, error};
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;

pub(crate) async fn torrent_event_loop(
    properties: Arc<Properties>,
    torrent_storage: TorrentStorage,
    torrent_process: Arc<TorrentToken>,
    mut broker_receiver: Receiver<TorrentEvent>,
) {
    let mut peer_manager = PeerManager::new(properties, torrent_storage, torrent_process)
        .expect("FIXME: need to turn this into non breaking failure");
    while let Some(event) = broker_receiver.next().await {
        debug!("received event: {}", event);
        match event {
            TorrentEvent::Announce(peers) => {
                peer_manager.peers_announced(peers).await;
            }
            TorrentEvent::PeerDisconnect(peer_id) => {
                if let Some(_peer_state) = peer_manager.peer_remove_by_id(peer_id) {
                    debug!("[{}] removed peer due to disconnect", peer_id);
                }
            }
            TorrentEvent::PeerConnectFailed(peer_id) => {
                if let Some(_peer_state) = peer_manager.peer_remove_by_id(peer_id) {
                    debug!("[{}] removed peer due to connection failure", peer_id);
                }
            }
            TorrentEvent::PeerForwarded(stream) => {
                if let Err(err) = peer_manager.peer_forwarded(stream).await {
                    error!("cannot forward peer: {}", err);
                }
            }
            TorrentEvent::PeerConnected(peer_id, stream) => {
                if let Err(err) = peer_manager.peer_connected(peer_id, stream).await {
                    error!("[{}] cannot process peer connected: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerPiece(peer_id, piece) => {
                if let Err(err) = peer_manager.peer_piece(peer_id, piece).await {
                    error!("[{}] cannot process peer piece: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerPieces(peer_id, pieces) => {
                if let Err(err) = peer_manager.peer_pieces(peer_id, pieces).await {
                    error!("[{}] cannot process peer pieces: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerUnchoke(peer_id) => {
                if let Err(err) = peer_manager.peer_unchoke(peer_id).await {
                    error!("[{}] cannot process peer unchoke: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerInterested(peer_id) => {
                if let Err(err) = peer_manager.peer_interested(peer_id).await {
                    error!("[{}] cannot process peer interested: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerPieceCanceled(peer_id) => {
                if let Err(err) = peer_manager.peer_piece_canceled(peer_id).await {
                    error!("[{}] cannot process peer piece canceled: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerPieceDownloaded(peer_id, piece) => {
                peer_manager.peer_piece_downloaded(peer_id, piece).await;
            }
            TorrentEvent::PeerPieceRequest {
                peer_id,
                index,
                begin,
                length,
            } => {
                if let Err(err) = peer_manager
                    .peer_piece_request(peer_id, index, begin, length)
                    .await
                {
                    error!("[{}] cannot process peer piece request: {}", peer_id, err);
                }
            }
            TorrentEvent::Enable(request_response) => {
                peer_manager.enable(request_response).await;
            }
            TorrentEvent::Disable(request_response) => {
                peer_manager.disable(request_response).await;
            }
            TorrentEvent::Subscribe(request_response) => {
                peer_manager.subscribe(request_response).await;
            }
            TorrentEvent::Delete(request_response) => {
                peer_manager.delete(request_response).await;
                break;
            }
            TorrentEvent::PeersView(request_response) => {
                peer_manager.peers_view(request_response).await;
            }
            TorrentEvent::AnnounceView(request_response) => {
                peer_manager.announce_view(request_response).await;
            }
            TorrentEvent::FilesView(request_response) => {
                peer_manager.files_view(request_response).await;
            }
            TorrentEvent::FileDownload(request_response) => {
                peer_manager.file_download(request_response).await;
            }
            TorrentEvent::QueryPiece(request_response) => {
                peer_manager.query_piece(request_response).await;
            }
        }
    }

    if let Err(err) = peer_manager.quit().await {
        error!("error during peer manager quit: {}", err);
    }

    debug!("download_torrent done");
}
