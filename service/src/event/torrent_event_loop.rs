use crate::{
    announce::{announce_loop, AnnounceManager, AnnounceManagerMessage},
    event::{TorrentDownloadMode, TorrentEvent, TorrentStatisticMessage},
    event_loop::EventLoop,
    peer::{PeerManager, PeerMessage, TorrentPeerState},
    process::TorrentToken,
    storage::TorrentStorage,
    types::{
        public::{AnnounceView, PeerView, TorrentDownloadState},
        Properties,
    },
    DEFAULT_CHANNEL_BUFFER,
};
use flat_storage::bit_by_index;
use futures::{
    future::{AbortHandle, Abortable},
    prelude::*,
    StreamExt,
};
use log::{debug, error};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{
    mpsc::{self, Receiver},
    watch,
};

pub(crate) async fn torrent_event_loop(
    properties: Arc<Properties>,
    torrent_storage: TorrentStorage,
    torrent_process: Arc<TorrentToken>,
    mut broker_receiver: Receiver<TorrentEvent>,
) {
    let (statistic_sender, mut statistic_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

    let mut torrent_download_state = {
        let storage_state = torrent_storage.receiver.borrow();
        TorrentDownloadState {
            downloaded: storage_state.bytes_write,
            uploaded: storage_state.bytes_read,
        }
    };

    let announce_manager = EventLoop::spawn(AnnounceManager {})
        .expect("FIXME: need to turn this into non breaking failure");

    let mut peer_manager = PeerManager {
        announce_manager,
        torrent_storage,
        torrent_process,
        peer_states: HashMap::new(),
        mode: TorrentDownloadMode::Normal,
        active: false,
        announce_abort_handle: None,
        awaiting_for_piece: HashMap::new(),
        statistic_sender,
    };

    let statistic_task = async move {
        let (watch_sender, watch_receiver) = watch::channel(torrent_download_state.clone());
        while let Some(message) = statistic_receiver.next().await {
            match message {
                TorrentStatisticMessage::Subscribe(request_response) => {
                    if let Err(err) = request_response.response(watch_receiver.clone()) {
                        error!(
                            "cannot send subscription response to torrent statistics: {}",
                            err
                        );
                    }
                }
                TorrentStatisticMessage::Uploaded(count) => {
                    torrent_download_state.uploaded += count;
                    if let Err(err) = watch_sender.broadcast(torrent_download_state) {
                        error!("cannot broadcast uploaded torrent statistics: {}", err);
                    }
                }
                TorrentStatisticMessage::Downloaded(count) => {
                    torrent_download_state.downloaded += count;
                    if let Err(err) = watch_sender.broadcast(torrent_download_state) {
                        error!("cannot broadcast downloaded torrent statistics: {}", err);
                    }
                }
                TorrentStatisticMessage::Quit => break,
            }
        }
    };
    tokio::spawn(statistic_task);

    while let Some(event) = broker_receiver.next().await {
        debug!("received event: {}", event);
        match event {
            TorrentEvent::Announce(peers) => {
                debug!("we got announce, what now?");
                for peer in peers {
                    debug!("peer announced: {:?}", peer);
                    if let Err(err) = peer_manager.peer_announced(peer.clone()).await {
                        error!("cannot process peer announced {:?}: {}", peer, err);
                    }
                }
            }
            TorrentEvent::PeerDisconnect(peer_id) => {
                if let Some(_peer_state) = peer_manager.peer_states.remove(&peer_id) {
                    debug!("[{}] removed peer due to disconnect", peer_id);
                }
            }
            TorrentEvent::PeerConnectFailed(peer_id) => {
                if let Some(_peer_state) = peer_manager.peer_states.remove(&peer_id) {
                    debug!("[{}] removed peer due to connection failure", peer_id);
                }
            }
            TorrentEvent::PeerForwarded(stream) => {
                debug!("peer forwarded");
                if let Err(err) = peer_manager.peer_forwarded(stream).await {
                    error!("cannot forward peer: {}", err);
                }
            }
            TorrentEvent::PeerConnected(peer_id, stream) => {
                debug!("[{}] peer connected to {:?}", peer_id, stream.peer_addr());
                if let Err(err) = peer_manager.peer_connected(peer_id, stream).await {
                    error!("[{}] cannot process peer connected: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerPiece(peer_id, piece) => {
                debug!("[{}] peer piece: {}", peer_id, piece);
                if let Err(err) = peer_manager.peer_piece(peer_id, piece).await {
                    error!("[{}] cannot process peer piece: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerPieces(peer_id, pieces) => {
                debug!("[{}] peer pieces", peer_id);
                if let Err(err) = peer_manager.peer_pieces(peer_id, pieces).await {
                    error!("[{}] cannot process peer pieces: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerUnchoke(peer_id) => {
                debug!("[{}] peer unchoke", peer_id);
                if let Err(err) = peer_manager.peer_unchoke(peer_id).await {
                    error!("[{}] cannot process peer unchoke: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerInterested(peer_id) => {
                debug!("[{}] peer interested", peer_id);
                if let Err(err) = peer_manager.peer_interested(peer_id).await {
                    error!("[{}] cannot process peer interested: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerPieceCanceled(peer_id) => {
                debug!("[{}] canceled piece for peer", peer_id);
                if let Err(err) = peer_manager.peer_piece_canceled(peer_id).await {
                    error!("[{}] cannot process peer piece canceled: {}", peer_id, err);
                }
            }
            TorrentEvent::PeerPieceDownloaded(peer_id, piece) => {
                debug!("[{}] downloaded piece for peer", peer_id);
                if let Err(err) = peer_manager
                    .peer_piece_downloaded(peer_id, piece.into())
                    .await
                {
                    error!(
                        "[{}] cannot process peer piece downloaded: {}",
                        peer_id, err
                    );
                }

                peer_manager.update_download_mode(peer_id);

                let pieces_left = peer_manager.torrent_storage.receiver.borrow().pieces_left;
                if pieces_left == 0 {
                    debug!(
                        "torrent downloaded, hash: {}",
                        percent_encode(&peer_manager.torrent_process.hash_id, NON_ALPHANUMERIC)
                    );
                } else {
                    debug!("pieces left: {}", pieces_left);
                }
            }
            TorrentEvent::PeerPieceRequest {
                peer_id,
                index,
                begin,
                length,
            } => {
                debug!("[{}] request piece to peer", peer_id);
                if let Err(err) = peer_manager
                    .peer_piece_request(peer_id, index, begin, length)
                    .await
                {
                    error!("[{}] cannot process peer piece request: {}", peer_id, err);
                }
            }
            TorrentEvent::Enable(request_response) => {
                if peer_manager.active {
                    if let Err(err) = request_response.response(Ok(())) {
                        error!("cannot send response for disable torrent: {}", err);
                    }
                    continue;
                }

                let (abort_handle, abort_registration) = AbortHandle::new_pair();

                let announce_loop = Abortable::new(
                    announce_loop(properties.clone(), peer_manager.torrent_process.clone())
                        .map_err(|e| {
                            error!("announce loop error: {}", e);
                            e
                        }),
                    abort_registration,
                );

                tokio::spawn(announce_loop);

                peer_manager.announce_abort_handle = Some(abort_handle);

                let result = peer_manager.enable().await;

                if let Err(err) = request_response.response(result) {
                    error!("cannot send response for enable torrent: {}", err);
                }
                peer_manager.active = true;
            }
            TorrentEvent::Disable(request_response) => {
                if !peer_manager.active {
                    if let Err(err) = request_response.response(Ok(())) {
                        error!("cannot send response for disable torrent: {}", err);
                    }
                    continue;
                }
                if let Some(abort_handle) = peer_manager.announce_abort_handle.take() {
                    abort_handle.abort();
                }

                for (peer_id, ref mut peer_state) in &mut peer_manager.peer_states {
                    match peer_state.state {
                        TorrentPeerState::Connected { ref mut sender, .. } => {
                            if let Err(err) = sender.send(PeerMessage::Disconnect).await {
                                error!(
                                    "[{}] disable torrent: cannot send disconnect message to peer: {}",
                                    peer_id, err
                                );
                            }
                        }
                        TorrentPeerState::Connecting(_) => {
                            error!("FIXME: need to stop cennecting too");
                        }
                        _ => (),
                    }
                }
                peer_manager.peer_states = HashMap::new();

                let result = peer_manager.disable().await;

                if let Err(err) = request_response.response(result) {
                    error!("cannot send response for disable torrent: {}", err);
                }
                peer_manager.active = false;
            }
            TorrentEvent::Subscribe(request_response) => {
                if let Err(err) = peer_manager
                    .statistic_sender
                    .send(TorrentStatisticMessage::Subscribe(request_response))
                    .await
                {
                    error!("cannot subscribe: {}", err);
                }
            }
            TorrentEvent::Delete(request_response) => {
                let delete_result = peer_manager
                    .torrent_storage
                    .delete(*request_response.request())
                    .await;

                if let Err(err) = request_response.response(delete_result) {
                    error!("cannot send response for delete torrent: {}", err);
                }
                break;
            }
            TorrentEvent::PeersView(request_response) => {
                let peers_view = peer_manager
                    .peer_states
                    .values()
                    .map(PeerView::from)
                    .collect();

                if let Err(err) = request_response.response(Ok(peers_view)) {
                    error!("cannot send response for delete torrent: {}", err);
                }
            }
            TorrentEvent::AnnounceView(request_response) => {
                if let Err(err) = request_response.response(Ok(vec![AnnounceView {
                    url: peer_manager.torrent_process.torrent.announce_url.clone(),
                }])) {
                    error!("cannot send response for delete torrent: {}", err);
                }
            }
            TorrentEvent::FilesView(request_response) => {
                let files_result = peer_manager.torrent_storage.files().await;

                if let Err(err) = request_response.response(files_result) {
                    error!("cannot send response for delete torrent: {}", err);
                }
            }
            TorrentEvent::FileDownload(request_response) => {
                debug!("processing file download");
                let (file_id, range) = request_response.request();
                let files_download = peer_manager
                    .torrent_storage
                    .download(*file_id, range.clone())
                    .await;

                if let Err(err) = request_response.response(files_download) {
                    error!("cannot send response for download torrent: {}", err);
                }
            }
            TorrentEvent::QueryPiece(request_response) => {
                debug!("query piece event: processing query piece");
                let request = request_response.request();
                let piece_index = request.piece;
                debug!("query piece event: search for piece index {}", piece_index);
                let piece_bit = {
                    let state = peer_manager.torrent_storage.receiver.borrow();
                    let downloaded = state.downloaded.as_slice();
                    bit_by_index(piece_index, downloaded)
                };
                if piece_bit.is_some() {
                    debug!("query piece event: found, loading from storage");
                    match peer_manager.torrent_storage.load(piece_index).await {
                        Ok(Some(piece)) => {
                            debug!("query piece event: loaded piece {}", piece.as_ref().len());
                            let waker = request.waker.lock().unwrap().take();
                            {
                                debug!("query piece event: sending piece to download stream");
                                if let Err(err) =
                                    request_response.response(Ok(piece.as_ref().into()))
                                {
                                    error!("cannot send response for query piece: {}", err);
                                    continue;
                                }
                            }

                            if let Some(waker) = waker {
                                debug!("query piece event: wake up waker");
                                waker.wake();
                            }
                            continue;
                        }
                        Ok(None) => {
                            error!("query piece event: no piece loaded");
                        }
                        Err(err) => {
                            error!("cannot load piece from storage: {}", err);
                            if let Err(err) = request_response.response(Err(err)) {
                                error!("cannot send response for query piece: {}", err);
                            }
                            continue;
                        }
                    }
                }
                debug!("query piece event: register awaiter");
                let awaiters = peer_manager
                    .awaiting_for_piece
                    .entry(piece_index)
                    .or_insert_with(|| vec![]);
                awaiters.push(request_response);
            }
        }
    }

    if let Err(err) = peer_manager
        .statistic_sender
        .send(TorrentStatisticMessage::Quit)
        .await
    {
        error!("cannot send quit to statistic: {}", err);
    }

    debug!("download_torrent done");
}
