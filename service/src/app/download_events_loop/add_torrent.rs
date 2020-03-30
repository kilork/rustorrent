use super::*;

pub(crate) async fn add_torrent(
    properties: Arc<Properties>,
    request: &RsbtCommandAddTorrent,
    id: &mut usize,
    torrents: &mut Vec<TorrentDownload>,
) -> Result<TorrentDownload, RsbtError> {
    let RsbtCommandAddTorrent {
        data,
        filename,
        state,
    } = request;
    debug!("we need to download {:?}", filename);
    let filepath = PathBuf::from(&filename);
    let name = filepath.file_stem().unwrap().to_string_lossy().into_owned();

    let torrent = parse_torrent(data)?;
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

    let torrent_header = TorrentDownloadHeader {
        file: filename.clone(),
        state: state.clone(),
    };
    let torrent_download = TorrentDownload {
        id: *id,
        name,
        header: torrent_header.clone(),
        process: torrent_process.clone(),
        properties: properties.clone(),
    };

    let torrent_storage = TorrentStorage::new(
        properties.clone(),
        filename.clone(),
        torrent_process.clone(),
    )
    .await?;

    save_current_torrents(properties.clone(), torrent_header).await?;

    torrents.push(torrent_download.clone());
    tokio::spawn(download_torrent(
        properties,
        torrent_storage,
        torrent_process.clone(),
        broker_receiver,
    ));

    if state == &TorrentDownloadState::Enabled {
        debug!("sending activation event");
        let (enable_request, response) = RequestResponse::new(());
        torrent_process
            .broker_sender
            .clone()
            .send(DownloadTorrentEvent::Enable(enable_request))
            .await?;
        response.await??;
        debug!("sending activation event done");
    }

    Ok(torrent_download)
}
