use super::*;

pub(crate) async fn add_torrent(
    properties: Arc<Properties>,
    request_response: &RequestResponse<Vec<u8>, Result<TorrentDownload, RsbtError>>,
    filename: String,
    state: TorrentDownloadState,
    id: &mut usize,
    torrents: &mut Vec<TorrentDownload>,
) -> Result<TorrentDownload, RsbtError> {
    debug!("we need to download {:?}", filename);
    let request = request_response.request();
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

    let torrent_header = TorrentDownloadHeader {
        file: filename.clone(),
        state,
    };
    let torrent_download = TorrentDownload {
        id: *id,
        name,
        header: torrent_header.clone(),
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

    if current_torrents
        .torrents
        .iter()
        .find(|x| x.file == filename)
        .is_none()
    {
        current_torrents.torrents.push(torrent_header);
        fs::write(torrents_toml, toml::to_string(&current_torrents)?).await?;
    }

    torrents.push(torrent_download.clone());
    tokio::spawn(download_torrent(
        properties.clone(),
        torrent_storage,
        torrent_process.clone(),
        broker_receiver,
    ));

    Ok(torrent_download)
}
