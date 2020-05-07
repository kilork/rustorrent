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
    announce_url: &str,
) -> Result<Announcement, RsbtError> {
    let addr = SocketAddr::new(properties.listen, properties.port);
    let udp_socket = UdpSocket::bind(addr).await?;

    let announce_url = &announce_url[UDP_PREFIX.len()..];
    debug!("connecting to {}", announce_url);

    let mut addrs = lookup_host(announce_url).await?;
    if let Some(addr) = addrs.next() {
        debug!("resolved addr: {}", addr);
        let mut udp_tracker_client = UdpTrackerClient::new(udp_socket, addr);

        udp_tracker_client
            .announce(properties, torrent_process)
            .await
    } else {
        Err(RsbtError::UdpTrackerImplementation)
    }
}
