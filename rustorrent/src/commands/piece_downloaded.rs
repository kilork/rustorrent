use super::*;
use crate::types::info::Piece;
use sha1::{Digest, Sha1};

impl Inner {
    pub(crate) fn command_piece_downloaded(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
        piece: usize,
    ) -> Result<(), RustorrentError> {
        info!("Piece {} downloaded, checking SHA1", piece);
        let torrent_storage = torrent_process.torrent_storage.read().unwrap();
        let torrent_pieces = &torrent_storage.pieces;
        if let Some(torrent_piece) = torrent_pieces.get(piece) {
            let piece_info = torrent_process
                .info
                .pieces
                .get(piece)
                .expect("Could not get SHA1 for piece");
            let mut piece_state = torrent_piece.lock().unwrap();
            let sha1: Piece = Sha1::digest(piece_state.data.as_slice())[..].try_into()?;
            if *piece_info != sha1 {
                warn!(
                    "Sha1 calculation is wrong, piece {} would be redownloaded",
                    piece
                );
                piece_state.downloaded = false;
                piece_state.init_from_info(&torrent_process.info, piece);
            } else {
                debug!("SHA1 for piece {} is ok", piece);

                let mut stats = torrent_process.stats.lock().unwrap();
                let len = piece_state.data.len();
                stats.downloaded += len;
                stats.left -= len;

                let piece_index = piece as u32;
                for peer in &torrent_storage.peers {
                    let state = peer.state.lock().unwrap();
                    if let TorrentPeerState::Connected { ref sender, .. } = *state {
                        debug!("Peer {}: sending message Have", peer.addr);
                        // TODO: according to design idea message should be send only from commands
                        // so we should refactor this later
                        crate::messages::send_message_to_peer(
                            sender,
                            Message::Have { piece_index },
                        );
                    }
                }

                if stats.left > 0 {
                    let download_next_block =
                        RustorrentCommand::DownloadNextBlock(torrent_process.clone(), torrent_peer);
                    self.send_command(download_next_block)?;
                } else {
                    // TODO: download finished - need to change state
                    debug!("Download finished.");
                    *torrent_process.torrent_state.lock().unwrap() = TorrentProcessState::Upload;
                }
            }
        } else {
            error!("Wrong piece index: {}", piece);
        }

        Ok(())
    }
}
