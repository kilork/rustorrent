use crate::{event_loop::EventLoopMessage, RsbtError};
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub(crate) struct EventLoopSender<M> {
    sender: Sender<EventLoopMessage<M>>,
}

impl<M> EventLoopSender<M> {
    fn new(sender: Sender<EventLoopMessage<M>>) -> Self {
        Self { sender }
    }

    pub(crate) async fn send<T: Into<EventLoopMessage<M>>>(
        &mut self,
        m: T,
    ) -> Result<(), RsbtError> {
        self.sender.send(m.into()).await?;

        Ok(())
    }
}

impl<M> From<Sender<EventLoopMessage<M>>> for EventLoopSender<M> {
    fn from(sender: Sender<EventLoopMessage<M>>) -> Self {
        Self::new(sender)
    }
}
