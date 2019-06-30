use super::*;

impl Inner {
    pub(crate) fn command_download_block(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
        block: Block,
    ) -> Result<(), RustorrentError> {
        info!("Received command to download block: {:?}", &block);

        if let TorrentPeerState::Connected {
            chocked,
            ref mut downloading,
            ref sender,
            ..
        } = *torrent_peer.state.lock().unwrap()
        {
            if !chocked && !*downloading {
                debug!("Peer {}: sending message Request", torrent_peer.addr);
                *downloading = true;
                crate::messages::send_message_to_peer(
                    sender,
                    Message::Request {
                        index: block.piece,
                        begin: block.begin,
                        length: block.length,
                    },
                );
            }
        }

        Ok(())
    }
}
