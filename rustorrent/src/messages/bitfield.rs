use super::*;

pub(crate) fn message_bitfield(
    torrent_process: Arc<TorrentProcess>,
    torrent_peer: Arc<TorrentPeer>,
    mut bitfield_pieces: Vec<u8>,
) -> Result<Option<RustorrentCommand>, RustorrentError> {
    let mut need_to_download = false;
    for (index, piece) in torrent_process
        .torrent_storage
        .read()
        .unwrap()
        .pieces
        .iter()
        .enumerate()
    {
        let downloaded = piece.lock().unwrap().downloaded;
        if downloaded {
            continue;
        }
        let (index_byte, index_bit) = index_in_bitarray(index);

        info!(
            "Piece {} is not downloaded, checking presence in bitfield ({}:{})",
            index, index_byte, index_bit
        );

        if let Some(v) = bitfield_pieces.get(index_byte).map(|&v| v & index_bit) {
            if v == index_bit {
                info!("Found piece to download from peer");
                need_to_download = true;
                break;
            }
        }
    }

    if let TorrentPeerState::Connected {
        ref mut pieces,
        chocked,
        ref sender,
        ..
    } = *torrent_peer.state.lock().unwrap()
    {
        pieces.clear();
        pieces.append(&mut bitfield_pieces);

        if chocked && need_to_download {
            debug!("Peer {}: sending message Interested", torrent_peer.addr);
            // TODO: according to design idea message should be send only from commands
            // so we should refactor this later
            send_message_to_peer(sender, Message::Interested);
        }
    }

    Ok(None)
}
