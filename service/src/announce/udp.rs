use crate::types::udp_tracker::{
    UdpTrackerCodec, UdpTrackerRequest, UdpTrackerResponse, UdpTrackerResponseData,
};
use crate::{errors::RsbtError, event::TorrentEvent, process::TorrentToken, types::Properties};
use futures::{SinkExt, StreamExt};
use log::{debug, error};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::net::UdpSocket;
use tokio::{net::lookup_host, time};
use tokio_util::udp::UdpFramed;

const UDP_PREFIX: &str = "udp://";

pub(crate) async fn udp_announce(
    properties: Arc<Properties>,
    torrent_process: Arc<TorrentToken>,
    announce_url: &str,
) -> Result<Duration, RsbtError> {
    let addr = SocketAddr::new(properties.listen, properties.port);
    let udp_socket = UdpSocket::bind(addr).await?;

    let announce_url = &announce_url[UDP_PREFIX.len()..];
    debug!("connecting to {}", announce_url);

    let (mut wtransport, mut rtransport) = UdpFramed::new(udp_socket, UdpTrackerCodec).split();

    // TODO: implement 2^n * 15 up to 8 times
    let mut addrs = lookup_host(announce_url).await?;
    if let Some(addr) = addrs.next() {
        let request = UdpTrackerRequest::connect();
        debug!("sending udp tracker connect request: {:?}", request);
        wtransport.send((request.clone(), addr)).await?;
        debug!("awaiting response...");
        match time::timeout(Duration::from_millis(500), rtransport.next()).await? {
            Some(Ok((connect_response, _socket))) => {
                debug!(
                    "received udp tracker connect response: {:?}",
                    connect_response
                );

                if !request.match_response(&connect_response) {
                    debug!("request does not match response");
                    return Ok(Duration::from_secs(5));
                }
                if let UdpTrackerResponse {
                    data: UdpTrackerResponseData::Connect { connection_id },
                    ..
                } = connect_response
                {
                    let request = UdpTrackerRequest::announce(
                        connection_id,
                        properties,
                        torrent_process.clone(),
                    );
                    debug!("sending udp tracker announce request: {:?}", request);
                    wtransport.send((request.clone(), addr)).await?;
                    if let Ok(Some(Ok((connect_response, _socket)))) =
                        time::timeout(Duration::from_millis(200), rtransport.next()).await
                    {
                        debug!(
                            "received udp tracker announce response: {:?}",
                            connect_response
                        );

                        if !request.match_response(&connect_response) {
                            debug!("request does not match response");
                            return Ok(Duration::from_secs(5));
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
                                .send(TorrentEvent::Announce(peers))
                                .await?;
                            return Ok(Duration::from_secs(interval as u64));
                        }
                    }
                }
            }
            Some(Err(err)) => error!("udp connect failure: {}", err),
            None => error!("no response from udp connect"),
        }
    }

    Ok(Duration::from_secs(5))
}
