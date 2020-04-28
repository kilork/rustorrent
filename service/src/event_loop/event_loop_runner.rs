use crate::{
    event_loop::{EventLoopMessage, EventLoopSender},
    RsbtError,
};
use async_trait::async_trait;

#[async_trait]
pub(crate) trait EventLoopRunner<M: Send + 'static, F: Send + 'static> {
    fn set_sender(&mut self, sender: EventLoopSender<M, F>);

    fn sender(&mut self) -> Option<&mut EventLoopSender<M, F>>;

    async fn start(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn handle(&mut self, _message: M) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn send<T: Send + 'static + Into<EventLoopMessage<M>>>(
        &mut self,
        m: T,
    ) -> Result<(), RsbtError> {
        if let Some(sender) = self.sender() {
            sender.send(m.into()).await?;
        }

        Ok(())
    }

    async fn feedback(&mut self, f: F) -> Result<(), RsbtError> {
        if let Some(sender) = self.sender() {
            sender.feedback(f).await?;
        }

        Ok(())
    }
}
