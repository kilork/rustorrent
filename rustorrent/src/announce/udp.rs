use super::*;

const UDP_PREFIX: &str = "udp://";

pub(crate) async fn udp_announce(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    announce_url: &str,
) -> Result<Duration, RustorrentError> {
    let config = &settings.config;

    let listen = config
        .listen
        .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));

    let addr = SocketAddr::new(listen.into(), config.port);
    let udp_socket = UdpSocket::bind(addr).await?;

    let announce_url = &announce_url[UDP_PREFIX.len()..];
    debug!("connecting to {}", announce_url);

    let stream = udp_socket.connect(announce_url).await?;

    // UdpFramed::new(stream, )

    Ok(Duration::from_secs(5))
}
