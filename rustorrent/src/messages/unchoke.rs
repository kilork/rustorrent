use super::*;

pub(crate) fn message_unchoke(
    torrent_process: Arc<TorrentProcess>,
    torrent_peer: Arc<TorrentPeer>,
) -> Result<Option<RustorrentCommand>, RustorrentError> {
    let mut state = torrent_peer.state.lock().unwrap();
    match *state {
        TorrentPeerState::Connected {
            ref mut chocked,
            ref interested,
            ref sender,
            ref pieces,
            ..
        } => {
            if !*chocked {
                warn!("Peer {}: already unchocked!", torrent_peer.addr);
            }
            *chocked = false;

            let torrent_pieces = &torrent_process.torrent_storage.read().unwrap().pieces;
            for (index, piece) in torrent_pieces.iter().enumerate() {
                let mut piece_state = piece.lock().unwrap();

                let downloaded = piece_state.downloaded;
                if downloaded {
                    continue;
                }

                if let Some((index_byte, index_bit)) = bit_by_index(index, &pieces) {
                    info!("Found piece to download from peer! And we can request, yahoo!");

                    let is_last_piece = index != torrent_pieces.len() - 1;

                    let info = &torrent_process.info;

                    let (piece_length, blocks_count) = if is_last_piece {
                        (info.piece_length, info.default_blocks_count)
                    } else {
                        (info.last_piece_length, info.last_piece_blocks_count)
                    };

                    // find block to request from peer
                    if piece_state.data.is_empty() {
                        piece_state.data = vec![0; piece_length];
                        piece_state.blocks = vec![0; (blocks_count / 8) + 1];
                        piece_state.blocks_to_download = blocks_count;
                    }

                    for block_index in 0..blocks_count {
                        if bit_by_index(block_index, &piece_state.blocks).is_none() {
                            let block = Block {
                                piece: index as u32,
                                begin: block_index as u32 * BLOCK_SIZE as u32,
                                length: BLOCK_SIZE as u32, // TODO: should be properly calculated for corner cases
                            };

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
