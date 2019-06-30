use super::*;

pub(crate) fn message_piece(
    torrent_process: Arc<TorrentProcess>,
    torrent_peer: Arc<TorrentPeer>,
    piece_index: u32,
    begin: u32,
    data: Vec<u8>,
) -> Result<Option<RustorrentCommand>, RustorrentError> {
    let mut blocks_downloading = torrent_process.blocks_downloading.lock().unwrap();

    if let TorrentPeerState::Connected {
        ref mut downloading,
        ..
    } = *torrent_peer.state.lock().unwrap()
    {
        if *downloading {
            debug!(
                "Peer {}: block download finished, releasing",
                torrent_peer.addr
            );

            *downloading = false;
        } else {
            warn!(
                "Peer {}: block download already finished, confusing",
                torrent_peer.addr
            )
        }
    }

    let block = Block {
        piece: piece_index,
        begin,
        length: data.len() as u32,
    };

    if let Some(another_torrent_peer) = blocks_downloading.remove(&block) {
        if let TorrentPeerState::Connected {
            ref mut downloading,
            ..
        } = *another_torrent_peer.state.lock().unwrap()
        {
            if *downloading {
                warn!(
                    "Peer {}: block download finished also there, releasing, but it is strange",
                    another_torrent_peer.addr
                );
                *downloading = false;
            }
        }
    } else {
        warn!("Cannot find block among downloading {:?}", block);
    }

    let storage = torrent_process.torrent_storage.read().unwrap();
    if let Some(piece) = storage.pieces.get(piece_index as usize) {
        let block_index = begin as usize / BLOCK_SIZE;
        let (block_index_byte, block_index_bit) = index_in_bitarray(block_index);

        let mut piece = piece.lock().unwrap();
        let begin = begin as usize;
        let to = &mut piece.data[begin..begin + data.len()];
        to.copy_from_slice(&data);
        if let Some(v) = piece.blocks.get_mut(block_index_byte) {
            *v |= block_index_bit;
        }

        piece.blocks_to_download -= 1;
        if piece.blocks_to_download == 0 {
            piece.downloaded = true;

            let piece_downloaded =
                RustorrentCommand::PieceDownloaded(torrent_process.clone(), piece_index as usize);
            return Ok(Some(piece_downloaded));
        }
    } else {
        panic!(
            "Cannot find piece with index {} to update with block data",
            piece_index
        );
    }

    Ok(None)
}
