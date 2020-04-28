use crate::{event_loop::EventLoopMessage, RsbtError};
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub(crate) struct EventLoopSender<M, F> {
    sender: Sender<EventLoopMessage<M, F>>,
}

impl<M, F> EventLoopSender<M, F> {
    fn new(sender: Sender<EventLoopMessage<M, F>>) -> Self {
        Self { sender }
    }

    pub(crate) async fn send<T: Into<EventLoopMessage<M, F>>>(
        &mut self,
        m: T,
    ) -> Result<(), RsbtError> {
        self.sender.send(m.into()).await?;

        Ok(())
    }

    pub(crate) async fn feedback(&mut self, f: F) -> Result<(), RsbtError> {
        self.sender.send(EventLoopMessage::Feedback(f)).await?;

        Ok(())
    }
}

impl<M, F> From<Sender<EventLoopMessage<M, F>>> for EventLoopSender<M, F> {
    fn from(sender: Sender<EventLoopMessage<M, F>>) -> Self {
        Self::new(sender)
    }
}
