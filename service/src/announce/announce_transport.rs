use crate::RsbtError;
use async_trait::async_trait;

#[async_trait]
pub trait AnnounceTransport: Clone + Default + Send + Sync + 'static {
    async fn request_announce(&self, url: String) -> Result<(), RsbtError>;
}
