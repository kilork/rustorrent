use super::*;

pub(crate) async fn add_to_current_torrents(
    properties: Arc<Properties>,
    torrent_header: TorrentDownloadHeader,
) -> Result<(), RsbtError> {
    let torrents_toml = properties.storage.join(TORRENTS_TOML);
    let mut current_torrents: CurrentTorrents = if torrents_toml.exists() {
        toml::from_str(&fs::read_to_string(&torrents_toml).await?)?
    } else {
        Default::default()
    };

    if let Some(current_torrent_header) = current_torrents
        .torrents
        .iter_mut()
        .find(|x| x.file == torrent_header.file)
    {
        *current_torrent_header = torrent_header;
    } else {
        current_torrents.torrents.push(torrent_header);
    }

    fs::write(torrents_toml, toml::to_string(&current_torrents)?).await?;

    Ok(())
}

pub(crate) async fn remove_from_current_torrents(
    properties: Arc<Properties>,
    torrent_header: TorrentDownloadHeader,
) -> Result<(), RsbtError> {
    let torrents_toml = properties.storage.join(TORRENTS_TOML);
    let mut current_torrents: CurrentTorrents = if torrents_toml.exists() {
        toml::from_str(&fs::read_to_string(&torrents_toml).await?)?
    } else {
        Default::default()
    };

    if let Some(position) = current_torrents
        .torrents
        .iter()
        .position(|x| x.file == torrent_header.file)
    {
        current_torrents.torrents.remove(position);
        fs::write(torrents_toml, toml::to_string(&current_torrents)?).await?;
    }

    Ok(())
}
