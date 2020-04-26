use crate::RsbtError;
use async_trait::async_trait;

#[async_trait]
pub(crate) trait EventLoopRunner {
    async fn start(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }
}
