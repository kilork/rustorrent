use super::*;

mod process_announce;
mod process_peer_announced;
mod process_peer_connected;
mod process_peer_forwarded;
mod process_peer_interested;
mod process_peer_piece;
mod process_peer_piece_canceled;
mod process_peer_piece_downloaded;
mod process_peer_piece_request;
mod process_peer_pieces;
mod process_peer_unchoke;

use process_announce::process_announce;
use process_peer_announced::process_peer_announced;
use process_peer_connected::process_peer_connected;
use process_peer_forwarded::process_peer_forwarded;
use process_peer_interested::process_peer_interested;
use process_peer_piece::process_peer_piece;
use process_peer_piece_canceled::process_peer_piece_canceled;
use process_peer_piece_downloaded::process_peer_piece_downloaded;
use process_peer_piece_request::process_peer_piece_request;
use process_peer_pieces::process_peer_pieces;
use process_peer_unchoke::process_peer_unchoke;

#[derive(Debug)]
pub(crate) enum DownloadTorrentEvent {
    Announce(Vec<Peer>),
    PeerAnnounced(Peer),
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
}

impl Display for DownloadTorrentEvent {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            DownloadTorrentEvent::PeerPieceDownloaded(uuid, data) => {
                write!(f, "PeerPieceDownloaded({}, [{}])", uuid, data.len())
            }
            _ => write!(f, "{:?}", self),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TorrentDownloadState {
    pub downloaded: u64,
    pub uploaded: u64,
}

pub enum TorrentStatisticMessage {
    Subscribe(RequestResponse<(), watch::Receiver<TorrentDownloadState>>),
    Downloaded(u64),
    Uploaded(u64),
    Quit,
}

pub(crate) async fn download_torrent(
    properties: Arc<Properties>,
    mut torrent_storage: TorrentStorage,
    torrent_process: Arc<TorrentProcess>,
    mut broker_receiver: Receiver<DownloadTorrentEvent>,
) {
    let mut peer_states = HashMap::new();
    let mut mode = TorrentDownloadMode::Normal;
    let mut active = false;
    let mut announce_abort_handle = None;

    let (mut statistic_sender, mut statistic_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

    let mut torrent_download_state = {
        let storage_state = torrent_storage.receiver.borrow();
        TorrentDownloadState {
            downloaded: storage_state.bytes_write,
            uploaded: storage_state.bytes_read,
        }
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
            DownloadTorrentEvent::Announce(peers) => {
                debug!("we got announce, what now?");
                spawn_and_log_error(process_announce(torrent_process.clone(), peers), || {
                    "process announce failed".to_string()
                });
            }
            DownloadTorrentEvent::PeerAnnounced(peer) => {
                debug!("peer announced: {:?}", peer);
                if let Err(err) =
                    process_peer_announced(torrent_process.clone(), &mut peer_states, peer.clone())
                        .await
                {
                    error!("cannot process peer announced {:?}: {}", peer, err);
                }
            }
            DownloadTorrentEvent::PeerDisconnect(peer_id) => {
                if let Some(_peer_state) = peer_states.remove(&peer_id) {
                    debug!("[{}] removed peer due to disconnect", peer_id);
                }
            }
            DownloadTorrentEvent::PeerConnectFailed(peer_id) => {
                if let Some(_peer_state) = peer_states.remove(&peer_id) {
                    debug!("[{}] removed peer due to connection failure", peer_id);
                }
            }
            DownloadTorrentEvent::PeerForwarded(stream) => {
                debug!("peer forwarded");
                if let Err(err) = process_peer_forwarded(
                    torrent_process.clone(),
                    &mut peer_states,
                    stream,
                    &mut torrent_storage,
                    statistic_sender.clone(),
                )
                .await
                {
                    error!("cannot forward peer: {}", err);
                }
            }
            DownloadTorrentEvent::PeerConnected(peer_id, stream) => {
                debug!("[{}] peer connected", peer_id);
                if let Err(err) = process_peer_connected(
                    torrent_process.clone(),
                    &mut peer_states,
                    peer_id,
                    stream,
                    statistic_sender.clone(),
                )
                .await
                {
                    error!("[{}] cannot process peer connected: {}", peer_id, err);
                }
            }
            DownloadTorrentEvent::PeerPiece(peer_id, piece) => {
                debug!("[{}] peer piece: {}", peer_id, piece);
                if let Err(err) = process_peer_piece(
                    &mut peer_states,
                    &mode,
                    peer_id,
                    piece,
                    &mut torrent_storage,
                )
                .await
                {
                    error!("[{}] cannot process peer piece: {}", peer_id, err);
                }
            }
            DownloadTorrentEvent::PeerPieces(peer_id, pieces) => {
                debug!("[{}] peer pieces", peer_id);
                if let Err(err) = process_peer_pieces(
                    &mut peer_states,
                    &mode,
                    peer_id,
                    pieces,
                    &mut torrent_storage,
                )
                .await
                {
                    error!("[{}] cannot process peer pieces: {}", peer_id, err);
                }
            }
            DownloadTorrentEvent::PeerUnchoke(peer_id) => {
                debug!("[{}] peer unchoke", peer_id);
                if let Err(err) = process_peer_unchoke(&mut peer_states, peer_id).await {
                    error!("[{}] cannot process peer unchoke: {}", peer_id, err);
                }
            }
            DownloadTorrentEvent::PeerInterested(peer_id) => {
                debug!("[{}] peer interested", peer_id);
                if let Err(err) = process_peer_interested(&mut peer_states, peer_id).await {
                    error!("[{}] cannot process peer interested: {}", peer_id, err);
                }
            }
            DownloadTorrentEvent::PeerPieceCanceled(peer_id) => {
                debug!("[{}] canceled piece for peer", peer_id);
                if let Err(err) = process_peer_piece_canceled(
                    &mut peer_states,
                    &mode,
                    peer_id,
                    &mut torrent_storage,
                )
                .await
                {
                    error!("[{}] cannot process peer piece canceled: {}", peer_id, err);
                }
            }
            DownloadTorrentEvent::PeerPieceDownloaded(peer_id, piece) => {
                debug!("[{}] downloaded piece for peer", peer_id);
                if let Err(err) = process_peer_piece_downloaded(
                    &mut peer_states,
                    &mode,
                    peer_id,
                    piece,
                    &mut torrent_storage,
                )
                .await
                {
                    error!(
                        "[{}] cannot process peer piece downloaded: {}",
                        peer_id, err
                    );
                }

                mode = determine_download_mode(&mut peer_states, &mut torrent_storage, peer_id);

                let pieces_left = torrent_storage.receiver.borrow().pieces_left;
                if pieces_left == 0 {
                    debug!(
                        "torrent downloaded, hash: {}",
                        percent_encode(&torrent_process.hash_id, NON_ALPHANUMERIC)
                    );
                } else {
                    debug!("pieces left: {}", pieces_left);
                }
            }
            DownloadTorrentEvent::PeerPieceRequest {
                peer_id,
                index,
                begin,
                length,
            } => {
                debug!("[{}] request piece to peer", peer_id);
                if let Err(err) = process_peer_piece_request(
                    &mut peer_states,
                    peer_id,
                    index,
                    begin,
                    length,
                    &mut torrent_storage,
                )
                .await
                {
                    error!("[{}] cannot process peer piece request: {}", peer_id, err);
                }
            }
            DownloadTorrentEvent::Enable(request_response) => {
                if active {
                    if let Err(err) = request_response.response(Ok(())) {
                        error!("cannot send response for disable torrent: {}", err);
                    }
                    continue;
                }

                let (abort_handle, abort_registration) = AbortHandle::new_pair();

                let announce_loop = Abortable::new(
                    announce::announce_loop(properties.clone(), torrent_process.clone()).map_err(
                        |e| {
                            error!("announce loop error: {}", e);
                            e
                        },
                    ),
                    abort_registration,
                );

                tokio::spawn(announce_loop);

                announce_abort_handle = Some(abort_handle);
                if let Err(err) = request_response.response(Ok(())) {
                    error!("cannot send response for enable torrent: {}", err);
                }
                active = true;
            }
            DownloadTorrentEvent::Disable(request_response) => {
                if !active {
                    if let Err(err) = request_response.response(Ok(())) {
                        error!("cannot send response for disable torrent: {}", err);
                    }
                    continue;
                }
                if let Some(abort_handle) = announce_abort_handle.take() {
                    abort_handle.abort();
                }

                for (peer_id, peer_state) in peer_states {
                    match peer_state.state {
                        TorrentPeerState::Connected { mut sender, .. } => {
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
                peer_states = HashMap::new();

                if let Err(err) = request_response.response(Ok(())) {
                    error!("cannot send response for disable torrent: {}", err);
                }
                active = false;
            }
            DownloadTorrentEvent::Subscribe(request_response) => {
                if let Err(err) = statistic_sender
                    .send(TorrentStatisticMessage::Subscribe(request_response))
                    .await
                {
                    error!("cannot subscribe: {}", err);
                }
            }
            DownloadTorrentEvent::Delete(request_response) => {
                let delete_result = torrent_storage.delete(*request_response.request()).await;

                if let Err(err) = request_response.response(delete_result) {
                    error!("cannot send response for delete torrent: {}", err);
                }
                break;
            }
        }
    }

    if let Err(err) = statistic_sender.send(TorrentStatisticMessage::Quit).await {
        error!("cannot send quit to statistic: {}", err);
    }

    debug!("download_torrent done");
}
