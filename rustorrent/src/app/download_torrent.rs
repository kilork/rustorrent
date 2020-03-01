use super::*;

pub(crate) async fn download_torrent(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    mut broker_receiver: Receiver<DownloadTorrentEvent>,
) -> Result<(), RustorrentError> {
    let mut torrent_storage = TorrentStorage::new(settings.clone(), torrent_process.clone());

    let (abort_handle, abort_registration) = AbortHandle::new_pair();

    let announce_loop = Abortable::new(
        announce::announce_loop(settings.clone(), torrent_process.clone()).map_err(|e| {
            error!("announce loop error: {}", e);
            e
        }),
        abort_registration,
    )
    .map_err(|e| {
        error!("abortable error: {}", e);
        e.into()
    });

    let mut peer_states = HashMap::new();
    let mut mode = TorrentDownloadMode::Normal;

    let download_torrent_events_loop = async move {
        while let Some(event) = broker_receiver.next().await {
            debug!("received event: {}", event);
            match event {
                DownloadTorrentEvent::Announce(peers) => {
                    debug!("we got announce, what now?");
                    spawn_and_log_error(
                        process_announce(settings.clone(), torrent_process.clone(), peers),
                        || format!("process announce failed"),
                    );
                }
                DownloadTorrentEvent::PeerAnnounced(peer) => {
                    debug!("peer announced: {:?}", peer);
                    if let Err(err) = process_peer_announced(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer.clone(),
                    )
                    .await
                    {
                        error!("cannot process peerannounced {:?}: {}", peer, err);
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
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        stream,
                        &mut torrent_storage,
                    )
                    .await
                    {
                        error!("cannot forward peer: {}", err);
                    }
                }
                DownloadTorrentEvent::PeerConnected(peer_id, stream) => {
                    debug!("[{}] peer connected", peer_id);
                    if let Err(err) = process_peer_connected(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                        stream,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer connected: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerPiece(peer_id, piece) => {
                    debug!("[{}] peer piece: {}", peer_id, piece);
                    if let Err(err) = process_peer_piece(
                        settings.clone(),
                        torrent_process.clone(),
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
                        settings.clone(),
                        torrent_process.clone(),
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
                    if let Err(err) = process_peer_unchoke(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer unchoke: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerInterested(peer_id) => {
                    debug!("[{}] peer interested", peer_id);
                    if let Err(err) = process_peer_interested(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer interested: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerPieceCanceled(peer_id) => {
                    debug!("[{}] canceled piece for peer", peer_id);
                    if let Err(err) = process_peer_piece_canceled(
                        settings.clone(),
                        torrent_process.clone(),
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
                        settings.clone(),
                        torrent_process.clone(),
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
                        settings.clone(),
                        torrent_process.clone(),
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
            }
        }

        abort_handle.abort();

        debug!("download events loop is done");

        Ok::<(), RustorrentError>(())
    };

    let _ = match try_join!(announce_loop, download_torrent_events_loop) {
        Ok(_) | Err(RustorrentError::Aborted) => debug!("download torrent is done"),
        Err(e) => error!("download torrent finished with failure: {}", e),
    };

    debug!("download_torrent done");

    Ok(())
}
