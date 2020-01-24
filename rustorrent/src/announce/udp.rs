use super::*;
use crate::types::udp_tracker::{
    UdpTrackerCodec, UdpTrackerRequest, UdpTrackerRequestData, UdpTrackerResponse,
    UdpTrackerResponseData,
};
use tokio::net::lookup_host;

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

    let (mut wtransport, mut rtransport) = UdpFramed::new(udp_socket, UdpTrackerCodec).split();

    loop {
        let mut addrs = lookup_host(announce_url).await?;
        if let Some(addr) = addrs.next() {
            let request = UdpTrackerRequest::connect();
            debug!("sending udp tracker connect request: {:?}", request);
            wtransport.send((request.clone(), addr)).await?;
            if let Some(Ok((connect_response, _socket))) = rtransport.next().await {
                debug!(
                    "received udp tracker connect response: {:?}",
                    connect_response
                );

                if !request.match_response(&connect_response) {
                    debug!("request does not match response");
                    break;
                }
                if let UdpTrackerResponse {
                    data: UdpTrackerResponseData::Connect { connection_id },
                    ..
                } = connect_response
                {
                    let request = UdpTrackerRequest::announce(
                        connection_id,
                        settings,
                        torrent_process.clone(),
                    );
                    debug!("sending udp tracker announce request: {:?}", request);
                    wtransport.send((request.clone(), addr)).await?;
                    if let Some(Ok((connect_response, _socket))) = rtransport.next().await {
                        debug!(
                            "received udp tracker announce response: {:?}",
                            connect_response
                        );

                        if !request.match_response(&connect_response) {
                            debug!("request does not match response");
                            break;
                        }
                        if let UdpTrackerResponse {
                            data:
                                UdpTrackerResponseData::Announce {
                                    interval, peers, ..
                                },
                            ..
                        } = connect_response
                        {
                            torrent_process
                                .broker_sender
                                .clone()
                                .send(DownloadTorrentEvent::Announce(peers))
                                .await?;
                            return Ok(Duration::from_secs(interval as u64));
                        }
                    }
                }
            }
        }
        break;
    }

    Ok(Duration::from_secs(5))
}
