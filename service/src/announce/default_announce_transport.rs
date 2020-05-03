use crate::{
    announce::{http, udp, Announce, AnnounceTransport, Announcement},
    process::TorrentToken,
    types::Properties,
    RsbtError,
};
use async_trait::async_trait;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct DefaultAnnounceTransport {
    properties: Arc<Properties>,
    torrent_token: Arc<TorrentToken>,
}

impl DefaultAnnounceTransport {}

#[async_trait]
impl AnnounceTransport for DefaultAnnounceTransport {
    fn new(properties: Arc<Properties>, torrent_token: Arc<TorrentToken>) -> Self {
        Self {
            properties,
            torrent_token,
        }
    }
    async fn request_announce(&self, url: String) -> Result<Announcement, RsbtError> {
        if let Some(proto) = url.split("://").next().map(|x| x.to_lowercase()) {
            match proto.as_str() {
                "http" | "https" => {
                    http::http_announce(self.properties.clone(), self.torrent_token.clone(), &url)
                        .await
                }
                "udp" => {
                    udp::udp_announce(self.properties.clone(), self.torrent_token.clone(), &url)
                        .await
                }
                "wss" | _ => Err(RsbtError::AnnounceProtocolUnknown(proto)),
            }
        } else {
            Err(RsbtError::AnnounceProtocolFailure)
        }
    }
}
