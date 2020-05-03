use crate::announce::AnnounceTransport;
use async_trait::async_trait;

#[derive(Clone, Default)]
pub(crate) struct DefaultAnnounceTransport;

#[async_trait]
impl AnnounceTransport for DefaultAnnounceTransport {
    async fn request_announce(&self, url: String) -> Result<(), crate::RsbtError> {
        todo!()
    }
}
