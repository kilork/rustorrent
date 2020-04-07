use super::*;

pub(crate) async fn process_peer_piece_downloaded(
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
    piece: Vec<u8>,
    storage: &mut TorrentStorage,
    awaiters: &mut HashMap<
        usize,
        Vec<RequestResponse<DownloadTorrentEventQueryPiece, Result<Vec<u8>, RsbtError>>>,
    >,
) -> Result<(), RsbtError> {
    debug!("[{}] peer piece downloaded", peer_id);

    let (index, new_pieces) = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        if let TorrentPeerState::Connected {
            ref pieces,
            ref mut downloading_piece,
            ref mut downloading_since,
            ref mut downloaded,
            ..
        } = existing_peer.state
        {
            *downloaded += piece.len();
            if let (Some(index), Some(_since)) =
                (downloading_piece.take(), downloading_since.take())
            {
                storage.save(index, piece.to_vec()).await?;

                let mut downloadable = vec![];
                for (i, &a) in pieces.iter().enumerate() {
                    match_pieces(
                        &mut downloadable,
                        &storage.receiver.borrow().downloaded,
                        i,
                        a,
                    );
                }
                (index, downloadable)
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    } else {
        return Ok(());
    };

    for (peer_id, peer_state) in peer_states.iter_mut().filter(|(&key, _)| key != peer_id) {
        if let TorrentPeerState::Connected {
            ref mut sender,
            ref pieces,
            ref mut downloading_piece,
            ..
        } = peer_state.state
        {
            let peer_already_have_piece = bit_by_index(index, pieces).is_some();
            if peer_already_have_piece {
                continue;
            }
            debug!("[{}] sending Have {}", peer_id, index);
            if let Err(err) = sender.send(PeerMessage::Have(index)).await {
                error!(
                    "[{}] cannot send Have to {:?}: {}",
                    peer_id, peer_state.peer, err
                );
            };

            let peer_downloads_same_piece = *downloading_piece == Some(index);
            if peer_downloads_same_piece {
                if let Err(err) = sender.send(PeerMessage::Cancel).await {
                    error!(
                        "[{}] cannot send Have to {:?}: {}",
                        peer_id, peer_state.peer, err
                    );
                };
            }
        }
    }

    select_new_peer(&new_pieces, peer_states, mode, peer_id).await?;

    if let Some(awaiters) = awaiters.remove(&index) {
        for awaiter in awaiters {
            let waker = awaiter.request().waker.lock().unwrap().take();
            if let Err(err) = awaiter.response(Ok(piece.to_vec())) {
                error!("cannot send to awaiter: {}", err);
            }
            if let Some(waker) = waker {
                waker.wake();
            }
        }
    }

    Ok(())
}
