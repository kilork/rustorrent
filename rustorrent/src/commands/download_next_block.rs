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
            debug!("Checking piece {}", piece_index);
            let mut piece_state = piece.lock().unwrap();

            let downloaded = piece_state.downloaded;
            if downloaded {
                continue;
            }

            let (piece_length, blocks_count) = info.sizes(piece_index);

            debug!(
                "Piece {} is not downdloaded, piece length: {}, blocks count: {}",
                piece_index, piece_length, blocks_count
            );

            if piece_state.data.is_empty() {
                piece_state.init(piece_length, blocks_count);
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
                                    let block = crate::messages::block_from_piece(
                                        piece_index,
                                        piece_length,
                                        block_index,
                                        blocks_count,
                                    );

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

        debug!("No next block to download found.");

        Ok(())
    }
}
