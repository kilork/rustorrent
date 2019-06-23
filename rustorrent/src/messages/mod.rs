use super::*;

use std::sync::Arc;
use std::sync::Mutex;

use futures::prelude::*;
use futures::sync::mpsc::{channel, Receiver, Sender};
use log::{debug, error, info, warn};

use crate::app::{TorrentPeer, TorrentPeerState, TorrentProcess};
use crate::errors::{RustorrentError, TryFromBencode};
use crate::types::message::Message;

pub(crate) fn message_bitfield(
    torrent_process: Arc<TorrentProcess>,
    torrent_peer: Arc<TorrentPeer>,
    mut bitfield_pieces: Vec<u8>,
) -> Result<(), RustorrentError> {
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
        let index_byte = index / 8;
        let index_bit = 128u8 >> (index % 8);

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
            let conntx = sender.clone();
            tokio::spawn(conntx.send(Message::Interested).map(|_| ()).map_err(|_| ()));
        }
    }
    Ok(())
}
