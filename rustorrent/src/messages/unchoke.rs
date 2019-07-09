use super::*;

pub(crate) fn message_unchoke(
    torrent_process: Arc<TorrentProcess>,
    torrent_peer: Arc<TorrentPeer>,
) -> Result<Option<RustorrentCommand>, RustorrentError> {
    let mut state = torrent_peer.state.lock().unwrap();
    match *state {
        TorrentPeerState::Connected {
            ref mut chocked,
            ref sender,
            ref mut pieces,
            ..
        } => {
            if !*chocked {
                warn!("Peer {}: already unchocked!", torrent_peer.addr);
            }
            *chocked = false;

            if pieces.is_empty() {
                debug!("Unchoke without Bitfield - init all pieces as present");
                let len = &torrent_process.info.len();
                let piece_length = count_parts(*len, 8);
                *pieces = vec![255; piece_length];
                debug!("Peer {}: sending message Interested", torrent_peer.addr);
                // TODO: according to design idea message should be send only from commands
                // so we should refactor this later
                send_message_to_peer(sender, Message::Interested);
                return Ok(Some(RustorrentCommand::DownloadNextBlock(
                    torrent_process.clone(),
                    torrent_peer.clone(),
                )));
            }

            let torrent_pieces = &torrent_process.torrent_storage.read().unwrap().pieces;
            for (index, piece) in torrent_pieces.iter().enumerate() {
                let mut piece_state = piece.lock().unwrap();

                let downloaded = piece_state.downloaded;
                if downloaded {
                    continue;
                }

                if let Some((index_byte, index_bit)) = bit_by_index(index, &pieces) {
                    info!("Found piece to download from peer! And we can request, yahoo!");

                    let info = &torrent_process.info;

                    let (piece_length, blocks_count) = info.sizes(index);

                    // find block to request from peer
                    if piece_state.data.is_empty() {
                        piece_state.init(piece_length, blocks_count);
                    }

                    for block_index in 0..blocks_count {
                        if bit_by_index(block_index, &piece_state.blocks).is_none() {
                            let block =
                                block_from_piece(index, piece_length, block_index, blocks_count);

                            let download_piece = RustorrentCommand::DownloadBlock(
                                torrent_process.clone(),
                                torrent_peer.clone(),
                                block,
                            );

                            return Ok(Some(download_piece));
                        }
                    }
                    break;
                }
            }
        }
        _ => warn!("Peer {} is in wrong state: {:?}", torrent_peer.addr, state),
    }

    Ok(None)
}