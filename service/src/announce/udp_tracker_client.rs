use crate::{
    announce::Announcement,
    process::{TorrentToken, TorrentTokenProvider},
    types::Properties,
    types::{
        udp_tracker::{
            UdpTrackerCodec, UdpTrackerRequest, UdpTrackerResponse, UdpTrackerResponseData,
        },
        PropertiesProvider, UdpTrackerCodecError,
    },
    RsbtError,
};
use futures::{future::BoxFuture, Sink, SinkExt, Stream, StreamExt};
use log::{debug, error};
use std::{
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{net::UdpSocket, time::timeout};
use tokio_util::udp::UdpFramed;

pub(crate) struct UdpTrackerClient<T = UdpFramed<UdpTrackerCodec>> {
    connection_id: Option<(Instant, i64)>,
    framed: T,
    addr: SocketAddr,
}

impl UdpTrackerClient {
    pub(crate) fn new(udp_socket: UdpSocket, addr: SocketAddr) -> Self {
        Self {
            connection_id: None,
            framed: UdpFramed::new(udp_socket, UdpTrackerCodec),
            addr,
        }
    }
}

impl<T> UdpTrackerClient<T>
where
    T: Stream<Item = Result<(UdpTrackerResponse, SocketAddr), UdpTrackerCodecError>>
        + Sink<(UdpTrackerRequest, SocketAddr), Error = UdpTrackerCodecError>
        + Unpin
        + Send,
{
    async fn connection_id<P, TT>(
        &mut self,
        properties: Arc<P>,
        torrent_token: Arc<TT>,
    ) -> Result<i64, RsbtError>
    where
        P: PropertiesProvider + Send + Sync + 'static,
        TT: TorrentTokenProvider + Send + Sync + 'static,
    {
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

    pub(crate) async fn announce<P, TT>(
        &mut self,
        properties: Arc<P>,
        torrent_token: Arc<TT>,
    ) -> Result<Announcement, RsbtError>
    where
        P: PropertiesProvider + Send + Sync + 'static,
        TT: TorrentTokenProvider + Send + Sync + 'static,
    {
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

    fn announce_request<P, TT>(
        &mut self,
        properties: Arc<P>,
        torrent_token: Arc<TT>,
    ) -> BoxFuture<'_, Result<UdpTrackerRequest, RsbtError>>
    where
        P: PropertiesProvider + Send + Sync + 'static,
        TT: TorrentTokenProvider + Send + Sync + 'static,
    {
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

    async fn send<P, TT>(
        &mut self,
        properties: Arc<P>,
        torrent_token: Arc<TT>,
        announce: bool,
    ) -> Result<UdpTrackerResponse, RsbtError>
    where
        P: PropertiesProvider + Send + Sync + 'static,
        TT: TorrentTokenProvider + Send + Sync + 'static,
    {
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

#[cfg(test)]
mod tests {

    use super::UdpTrackerClient;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    struct TestUdpFramed {}

    #[tokio::test]
    async fn udp_tracker_client() {
        /*
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let udp_tracker_client = UdpTrackerClient {
            connection_id: None,
            framed: TestUdpFramed {},
            addr,
        };
        */
    }
}