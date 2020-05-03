use crate::{announce::Announcement, process::TorrentToken, types::Properties, RsbtError};
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub(crate) trait AnnounceTransport: Clone + Send + Sync + 'static {
    fn new(properties: Arc<Properties>, torrent_token: Arc<TorrentToken>) -> Self;
    async fn request_announce(&self, url: String) -> Result<Announcement, RsbtError>;
}
