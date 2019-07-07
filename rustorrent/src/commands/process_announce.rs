use super::*;

impl Inner {
    pub(crate) fn command_process_announce(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        tracker_announce: TrackerAnnounce,
    ) -> Result<(), RustorrentError> {
        info!("time to process announce");
        match *torrent_process.announce_state.lock().unwrap() {
            AnnounceState::Idle => {
                self.clone().spawn_delayed_announce(
                    torrent_process.clone(),
                    Duration::from_secs(tracker_announce.interval as u64),
                )?;
            }
            AnnounceState::Error(ref error) => {
                return Err(RustorrentError::FailureReason(format!(
                    "Announce failure: {}",
                    error
                )))
            }
            ref state => {
                return Err(RustorrentError::FailureReason(format!(
                    "Wrong state: {:?}",
                    state
                )))
            }
        }

        let mut torrent_storage = torrent_process.torrent_storage.write().unwrap();
        for peer in &tracker_announce.peers {
            let addr = SocketAddr::new(peer.ip, peer.port);
            if let Some(existing_peer) = torrent_storage
                .peers
                .iter()
                .filter(|x| x.addr == addr)
                .next()
            {
                info!("Checking peer: {:?}", peer);
                let announcement_count = existing_peer
                    .announcement_count
                    .fetch_add(1, Ordering::SeqCst);
                debug!(
                    "Peer {:?} announced {} time(s)",
                    peer,
                    announcement_count + 1
                );
                match *existing_peer.state.lock().unwrap() {
                    TorrentPeerState::Idle => {
                        info!("Reconnecting to peer: {:?}", peer);

                        let connect_to_peer = RustorrentCommand::ConnectToPeer(
                            torrent_process.clone(),
                            existing_peer.clone(),
                        );
                        self.clone().send_command(connect_to_peer)?;
                    }
                    _ => (),
                }
            } else {
                info!("Adding peer: {:?}", peer);
                let peer: Arc<TorrentPeer> = Arc::new(peer.into());
                torrent_storage.peers.push(peer.clone());
                let connect_to_peer =
                    RustorrentCommand::ConnectToPeer(torrent_process.clone(), peer);
                self.clone().send_command(connect_to_peer)?;
            }
        }

        Ok(())
    }
}
