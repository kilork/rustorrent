use super::*;

pub(crate) fn message_unchoke(
    torrent_process: Arc<TorrentProcess>,
    torrent_peer: Arc<TorrentPeer>,
) -> Result<(), RustorrentError> {
    match *torrent_peer.state.lock().unwrap() {
        TorrentPeerState::Connected {
            ref mut chocked,
            ref interested,
            ref sender,
            ref pieces,
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

                let (index_byte, index_bit) = index_in_bitarray(index);

                info!(
                    "Piece {} is not downloaded, checking presence in bitfield ({}:{})",
                    index, index_byte, index_bit
                );

                if let Some(v) = pieces.get(index_byte).map(|&v| v & index_bit) {
                    if v == index_bit {
                        info!("Found piece to download from peer! And we can request, yahoo!");

                        let is_last_piece = index != torrent_pieces.len() - 1;
                        let info = &torrent_process.info;
                        // find block to request from peer
                        if piece_state.data.is_empty() {
                            let (piece_length, blocks_count) = if is_last_piece {
                                (info.piece_length, info.default_blocks_count)
                            } else {
                                (info.last_piece_length, info.last_piece_blocks_count)
                            };
                            piece_state.data = vec![0; piece_length];
                            piece_state.blocks = vec![0; blocks_count];
                        }

                        break;
                    }
                }
            }
        }
        _ => (),
    }

    Ok(())
}
