use crate::{
    announce::Announcement,
    process::TorrentToken,
    types::udp_tracker::{
        UdpTrackerCodec, UdpTrackerRequest, UdpTrackerResponse, UdpTrackerResponseData,
    },
    types::Properties,
    RsbtError,
};
use futures::{future::BoxFuture, SinkExt, StreamExt};
use log::{debug, error};
use std::{
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::net::UdpSocket;
use tokio::{net::lookup_host, time::timeout};
use tokio_util::udp::UdpFramed;

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

struct UdpTrackerClient {
    connection_id: Option<(Instant, i64)>,
    framed: UdpFramed<UdpTrackerCodec>,
    addr: SocketAddr,
}

impl UdpTrackerClient {
    fn new(udp_socket: UdpSocket, addr: SocketAddr) -> Self {
        Self {
            connection_id: None,
            framed: UdpFramed::new(udp_socket, UdpTrackerCodec),
            addr,
        }
    }

    async fn connection_id(
        &mut self,
        properties: Arc<Properties>,
        torrent_token: Arc<TorrentToken>,
    ) -> Result<i64, RsbtError> {
        debug!("connect");
        let connection_id = self
            .connection_id
            .filter(|(received, _)| received.elapsed() < Duration::from_secs(60))
            .map(|(_, id)| id);

        if let Some(connection_id) = connection_id {
            return Ok(connection_id);
        }

        if let UdpTrackerResponse {
            data: UdpTrackerResponseData::Connect { connection_id },
            ..
        } = self.send(properties, torrent_token, false).await?
        {
            self.connection_id = Some((Instant::now(), connection_id));
            Ok(connection_id)
        } else {
            Err(RsbtError::UdpTrackerImplementation)
        }
    }

    async fn announce(
        &mut self,
        properties: Arc<Properties>,
        torrent_token: Arc<TorrentToken>,
    ) -> Result<Announcement, RsbtError> {
        debug!("announce");
        if let UdpTrackerResponse {
            data:
                UdpTrackerResponseData::Announce {
                    interval, peers, ..
                },
            ..
        } = self.send(properties, torrent_token, true).await?
        {
            Ok(Announcement {
                peers,
                requery_interval: Duration::from_secs(interval as u64),
            })
        } else {
            Err(RsbtError::UdpTrackerImplementation)
        }
    }

    async fn connect_request(&mut self) -> Result<UdpTrackerRequest, RsbtError> {
        Ok(UdpTrackerRequest::connect())
    }

    fn announce_request(
        &mut self,
        properties: Arc<Properties>,
        torrent_token: Arc<TorrentToken>,
    ) -> BoxFuture<'_, Result<UdpTrackerRequest, RsbtError>> {
        Box::pin(async {
            let connection_id = Pin::new(self)
                .connection_id(properties.clone(), torrent_token.clone())
                .await?;
            Ok(UdpTrackerRequest::announce(
                connection_id,
                properties,
                torrent_token,
            ))
        })
    }

    async fn send(
        &mut self,
        properties: Arc<Properties>,
        torrent_token: Arc<TorrentToken>,
        announce: bool,
    ) -> Result<UdpTrackerResponse, RsbtError> {
        let addr = self.addr;
        for n in 0..=8 {
            let request = if announce {
                self.announce_request(properties.clone(), torrent_token.clone())
                    .await
            } else {
                self.connect_request().await
            };
            let request = match request {
                Ok(request) => request,
                Err(err) => {
                    error!("request error: {}", err);
                    continue;
                }
            };
            self.framed.send((request.clone(), addr)).await?;
            let loss_threshold = Duration::from_secs(2u64.pow(n) * 15);
            match timeout(loss_threshold, self.framed.next()).await {
                Ok(Some(Ok((response, _)))) => {
                    if !request.match_response(&response) {
                        debug!("udp connection request does not match response");
                        continue;
                    }
                    return Ok(response);
                }
                Ok(Some(Err(err))) => {
                    error!("udp connection error: {}", err);
                }
                Ok(None) => {
                    debug!("udp connection dropped");
                }
                Err(res) => {
                    debug!("udp connection timeout: {:?} {:?}", loss_threshold, res);
                }
            }
        }
        error!("udp connection timeout");
        Err(RsbtError::UdpTrackerTimeout)
    }
}
