use super::*;

const UDP_PREFIX: &str = "udp://";

pub(crate) async fn udp_announce(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    announce_url: &str,
) -> Result<Duration, RustorrentError> {
    Ok(Duration::from_secs(5))
}
