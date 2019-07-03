use super::*;

impl Inner {
    pub(crate) fn command_download_block(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
        block: Block,
    ) -> Result<(), RustorrentError> {
        info!("Received command to download block: {:?}", &block);

        {
            let mut torrent_process_state = torrent_process.torrent_state.lock().unwrap();
            *torrent_process_state = match *torrent_process_state {
                TorrentProcessState::Init => TorrentProcessState::Download,
                TorrentProcessState::Download => TorrentProcessState::Download,
                TorrentProcessState::DownloadUpload => TorrentProcessState::DownloadUpload,
                TorrentProcessState::Upload => TorrentProcessState::DownloadUpload,
                TorrentProcessState::Finished => TorrentProcessState::DownloadUpload,
            };
        }
        let mut blocks_downloading = torrent_process.blocks_downloading.lock().unwrap();

        if let Some(another_torrent_peer) = blocks_downloading.get(&block) {
            if let TorrentPeerState::Connected { downloading, .. } =
                *another_torrent_peer.state.lock().unwrap()
            {
                if downloading {
                    debug!(
                        "Another peer {} downloading same {:?}",
                        another_torrent_peer.addr, &block
                    );

                    return Ok(());
                }
            }
        }

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
                let request = Message::Request {
                    index: block.piece,
                    begin: block.begin,
                    length: block.length,
                };
                blocks_downloading.insert(block, torrent_peer.clone());
                crate::messages::send_message_to_peer(sender, request);
            }
        }

        Ok(())
    }
}
