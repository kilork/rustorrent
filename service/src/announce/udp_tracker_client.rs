use crate::{
    announce::Announcement,
    process::TorrentTokenProvider,
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
    use crate::{
        process::TorrentTokenProvider,
        types::{
            info::TorrentInfo, udp_tracker::UdpTrackerRequest, PropertiesProvider,
            UdpTrackerCodecError, UdpTrackerResponse, UdpTrackerResponseData,
        },
        RsbtError,
    };
    use futures::{Sink, Stream, StreamExt};
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        pin::Pin,
        sync::Arc,
        task::{Context, Poll},
    };

    struct TestUdpFramed {
        transaction_id: i32,
        responses: Vec<Result<UdpTrackerResponse, UdpTrackerCodecError>>,
        addr: SocketAddr,
    }

    impl Stream for TestUdpFramed {
        type Item = Result<(UdpTrackerResponse, SocketAddr), UdpTrackerCodecError>;
        fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let pin = self.get_mut();
            Poll::Ready(pin.responses.pop().map(|x| {
                x.map(|mut y| {
                    y.transaction_id = pin.transaction_id;
                    (y, pin.addr.clone())
                })
            }))
        }
    }

    impl Sink<(UdpTrackerRequest, SocketAddr)> for TestUdpFramed {
        type Error = UdpTrackerCodecError;
        fn poll_ready(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn start_send(
            self: Pin<&mut Self>,
            item: (UdpTrackerRequest, SocketAddr),
        ) -> Result<(), Self::Error> {
            let pin = self.get_mut();
            pin.transaction_id = item.0.transaction_id;
            Ok(())
        }
        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn poll_close(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    struct TestProperties;

    impl PropertiesProvider for TestProperties {
        fn port(&self) -> u16 {
            9999
        }
    }

    struct TestTorrentToken(TorrentInfo);

    impl TorrentTokenProvider for TestTorrentToken {
        fn info(&self) -> &TorrentInfo {
            &self.0
        }

        fn hash_id(&self) -> &[u8; crate::SHA1_SIZE] {
            &[0; crate::SHA1_SIZE]
        }
    }

    #[tokio::test]
    async fn udp_tracker_client_announce() {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        let mut udp_tracker_client = UdpTrackerClient {
            connection_id: None,
            framed: test_udp_frame(),
            addr,
        };
        let properties = Arc::new(TestProperties);
        let torrent_token = Arc::new(TestTorrentToken(TorrentInfo {
            piece_length: 0,
            default_blocks_count: 0,
            last_piece_length: 0,
            last_piece_blocks_count: 0,
            pieces: vec![],
            length: 100,
            files: vec![],
        }));

        let announcement = udp_tracker_client
            .announce(properties, torrent_token)
            .await
            .expect("udp tracker announcement");

        assert_eq!(announcement.peers.len(), 1);
    }

    fn test_udp_frame() -> TestUdpFramed {
        TestUdpFramed {
            transaction_id: 0,
            responses: vec![
                Ok(UdpTrackerResponse {
                    transaction_id: 1,
                    data: UdpTrackerResponseData::Announce {
                        interval: 600,
                        leechers: 1,
                        seeders: 2,
                        peers: vec![SocketAddr::new(
                            IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1)),
                            9999,
                        )
                        .into()],
                    },
                }),
                Ok(UdpTrackerResponse {
                    transaction_id: 1,
                    data: UdpTrackerResponseData::Connect { connection_id: 0 },
                }),
            ],
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
        }
    }

    #[tokio::test]
    async fn check_test_udp_frame() {
        let mut test_udp_frame = test_udp_frame();
        let mut count: usize = 0;
        while let Some(Ok(_message)) = test_udp_frame.next().await {
            count += 1;
        }
        assert_eq!(2, count);
    }
}
