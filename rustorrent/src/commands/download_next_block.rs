use super::*;

impl Inner {
    pub(crate) fn command_download_next_block(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
    ) -> Result<(), RustorrentError> {
        info!("Received command to download next block");

        let torrent_storage = torrent_process.torrent_storage.read().unwrap();
        let torrent_pieces = &torrent_storage.pieces;
        let torrent_peers = &torrent_storage.peers;
        let info = &torrent_process.info;

        for (piece_index, piece) in torrent_pieces.iter().enumerate() {
            let mut piece_state = piece.lock().unwrap();

            let downloaded = piece_state.downloaded;
            if downloaded {
                continue;
            }

            let is_last_piece = piece_index != torrent_pieces.len() - 1;

            let (piece_length, blocks_count) = if is_last_piece {
                (info.piece_length, info.default_blocks_count)
            } else {
                (info.last_piece_length, info.last_piece_blocks_count)
            };

            if piece_state.data.is_empty() {
                piece_state.data = vec![0; piece_length];
                piece_state.blocks = vec![0; (blocks_count / 8) + 1];
                piece_state.blocks_to_download = blocks_count;
            }

            for peer in torrent_peers {
                let state = peer.state.lock().unwrap();
                match *state {
                    TorrentPeerState::Connected {
                        chocked,
                        ref pieces,
                        ..
                    } => {
                        if chocked {
                            continue;
                        }
                        if crate::messages::bit_by_index(piece_index, &pieces).is_some() {
                            debug!("Found possible candidate to download: {}", piece_index);
                            for block_index in 0..blocks_count {
                                if crate::messages::bit_by_index(block_index, &piece_state.blocks)
                                    .is_none()
                                {
                                    let block = Block {
                                        piece: piece_index as u32,
                                        begin: block_index as u32 * BLOCK_SIZE as u32,
                                        length: BLOCK_SIZE as u32, // TODO: should be properly calculated for corner cases
                                    };

                                    let download_piece = RustorrentCommand::DownloadBlock(
                                        torrent_process.clone(),
                                        peer.clone(),
                                        block,
                                    );
                                    self.clone().send_command(download_piece)?;
                                    return Ok(());
                                }
                            }
                        }
                    }
                    _ => (),
                }
            }
        }

        Ok(())
    }
}
