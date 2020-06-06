use crate::{
    announce::{Announcement, UdpTrackerClient},
    process::TorrentToken,
    types::Properties,
    RsbtError,
};
use log::debug;
use std::{net::SocketAddr, sync::Arc};
use tokio::net::lookup_host;
use tokio::net::UdpSocket;

const UDP_PREFIX: &str = "udp://";

pub(crate) async fn udp_announce(
    properties: Arc<Properties>,
    torrent_process: Arc<TorrentToken>,
    announce_url: String,
) -> Result<Announcement, RsbtError> {
    let addr = SocketAddr::new(properties.listen, properties.port);
    let udp_socket = UdpSocket::bind(addr).await?;

    let connect_url = &announce_url[UDP_PREFIX.len()..].to_string();
    debug!("connecting to {}", connect_url);

    let mut addrs = lookup_host(connect_url).await?;
    if let Some(addr) = addrs.next() {
        debug!("resolved addr: {}", addr);
        let mut udp_tracker_client = UdpTrackerClient::new(udp_socket, announce_url, addr);

        udp_tracker_client
            .announce(properties, torrent_process)
            .await
    } else {
        Err(RsbtError::UdpTrackerImplementation)
    }
}
