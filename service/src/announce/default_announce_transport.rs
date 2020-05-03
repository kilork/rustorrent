use crate::{
    announce::{AnnounceTransport, Announcement},
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
        todo!()
    }
}
