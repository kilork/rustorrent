use super::*;
use std::path::PathBuf;

pub(crate) async fn download_events_loop(
    properties: Arc<Properties>,
    mut events: Receiver<RsbtCommand>,
) {
    let mut torrents = vec![];
    let mut id = 0;

    while let Some(event) = events.next().await {
        match event {
            RsbtCommand::AddTorrent(request_response, filename) => {
                let torrent = add_torrent(
                    properties.clone(),
                    &request_response,
                    filename,
                    &mut id,
                    &mut torrents,
                )
                .await;
                if let Err(err) = request_response.response(torrent) {
                    error!("cannot send response for add torrent: {}", err);
                }
            }
            RsbtCommand::TorrentHandshake {
                handshake_request,
                handshake_sender,
            } => {
                debug!("searching for matching torrent handshake");

                let hash_id = handshake_request.info_hash;

                if handshake_sender
                    .send(
                        torrents
                            .iter()
                            .map(|x| &x.process)
                            .find(|x| x.hash_id == hash_id)
                            .cloned(),
                    )
                    .is_err()
                {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
            RsbtCommand::TorrentList { sender } => {
                debug!("collecting torrent list");
                if sender.send(torrents.to_vec()).is_err() {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
        }
    }

    debug!("download_events_loop done");
}

async fn add_torrent(
    properties: Arc<Properties>,
    request_response: &RequestResponse<Vec<u8>, Result<TorrentDownload, RsbtError>>,
    filename: String,
    id: &mut usize,
    torrents: &mut Vec<TorrentDownload>,
) -> Result<TorrentDownload, RsbtError> {
    debug!("we need to download {:?}", filename);
    if let Some(request) = request_response.request() {
        let filepath = PathBuf::from(&filename);
        let name = filepath.file_stem().unwrap().to_string_lossy().into_owned();

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

        *id += 1;

        let torrent_process = Arc::new(TorrentProcess {
            info,
            hash_id,
            torrent,
            handshake,
            broker_sender,
        });

        let torrent_download = TorrentDownload {
            id: *id,
            name,
            active: true,
            process: torrent_process.clone(),
        };

        let torrent_storage = TorrentStorage::new(
            properties.clone(),
            filename.clone(),
            torrent_process.clone(),
        )
        .await?;

        let torrents_toml = properties.storage.join(TORRENTS_TOML);
        let mut current_torrents: CurrentTorrents = if torrents_toml.exists() {
            toml::from_str(&fs::read_to_string(&torrents_toml).await?)?
        } else {
            Default::default()
        };

        if !current_torrents.torrents.contains(&filename) {
            current_torrents.torrents.push(filename);
            fs::write(torrents_toml, toml::to_string(&current_torrents)?).await?;
        }

        torrents.push(torrent_download.clone());
        tokio::spawn(download_torrent(
            properties.clone(),
            torrent_storage,
            torrent_process.clone(),
            broker_receiver,
        ));

        return Ok(torrent_download);
    }

    panic!();
}
