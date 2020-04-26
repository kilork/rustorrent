use crate::{event_loop::EventLoopSender, RsbtError};
use async_trait::async_trait;

#[async_trait]
pub(crate) trait EventLoopRunner<M: Send + 'static> {
    async fn start(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn handle(
        &mut self,
        _message: M,
        _event_loop_sender: &mut EventLoopSender<M>,
    ) -> Result<(), RsbtError> {
        Ok(())
    }
}
