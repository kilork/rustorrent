use super::*;

pub(crate) async fn process_peer_forwarded(
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    stream: TcpStream,
    storage: &mut TorrentStorage,
    statistic_sender: Sender<TorrentStatisticMessage>,
) -> Result<(), RsbtError> {
    let peer_id = Uuid::new_v4();
    debug!("[{}] peer connection forwarded", peer_id);

    let peer_addr = stream.peer_addr()?;

    let peer: Peer = peer_addr.into();

    let (mut sender, receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

    peer_states.insert(
        peer_id,
        PeerState {
            peer: peer.clone(),
            state: TorrentPeerState::Connected {
                chocked: true,
                interested: false,
                downloading_piece: None,
                downloading_since: None,
                downloaded: 0,
                uploaded: 0,
                pieces: vec![],
                sender: sender.clone(),
            },
            announce_count: 0,
        },
    );

    {
        let downloaded = storage.receiver.borrow().downloaded.clone();
        if !downloaded.is_empty() {
            sender.send(PeerMessage::Bitfield(downloaded)).await?;
        }
    }

    let _ = spawn_and_log_error(
        peer_loop(
            torrent_process,
            peer_id,
            sender,
            receiver,
            stream,
            statistic_sender,
        ),
        move || format!("[{}] peer loop failed", peer_id),
    );

    Ok(())
}
