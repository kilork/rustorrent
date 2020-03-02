use super::*;

pub(crate) async fn download_events_loop(
    settings: Arc<Settings>,
    mut events: Receiver<RustorrentCommand>,
) -> Result<(), RustorrentError> {
    let mut torrents = vec![];
    let mut id = 0;

    while let Some(event) = events.next().await {
        match event {
            RustorrentCommand::AddTorrent(request_response, filename) => {
                debug!("we need to download {:?}", filename);
                if let Some(request) = request_response.request() {
                    let torrent = parse_torrent(request)?;
                    let hash_id = torrent.info_sha1_hash();
                    let info = torrent.info()?;

                    debug!("torrent size: {}", info.len());
                    debug!("piece length: {}", info.piece_length);
                    debug!("total pieces: {}", info.pieces.len());

                    let mut handshake = vec![];
                    handshake.extend_from_slice(&crate::types::HANDSHAKE_PREFIX);
                    handshake.extend_from_slice(&hash_id);
                    handshake.extend_from_slice(&PEER_ID);

                    let (broker_sender, broker_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);
                    id += 1;
                    let torrent_process = Arc::new(TorrentProcess {
                        id,
                        filename,
                        info,
                        hash_id,
                        torrent,
                        handshake,
                        broker_sender,
                    });

                    torrents.push(torrent_process.clone());

                    let _ = spawn_and_log_error(
                        download_torrent(
                            settings.clone(),
                            torrent_process.clone(),
                            broker_receiver,
                        ),
                        || "download_events_loop: add torrent failed".to_string(),
                    );

                    if let Err(err) = request_response.response(Ok(torrent_process)) {
                        error!("cannot send response for add torrent: {}", err);
                    }
                }
            }
            RustorrentCommand::TorrentHandshake {
                handshake_request,
                handshake_sender,
            } => {
                debug!("searching for matching torrent handshake");

                let hash_id = handshake_request.info_hash;

                if handshake_sender
                    .send(torrents.iter().find(|x| x.hash_id == hash_id).cloned())
                    .is_err()
                {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
            RustorrentCommand::TorrentList { sender } => {
                debug!("collecting torrent list");
                if sender.send(torrents.to_vec()).is_err() {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
        }
    }

    debug!("download_events_loop done");

    Ok(())
}
