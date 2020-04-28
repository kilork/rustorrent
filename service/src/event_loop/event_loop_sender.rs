use crate::{event_loop::EventLoopMessage, RsbtError};
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub(crate) struct EventLoopSender<F, M> {
    sender: Sender<EventLoopMessage<F, M>>,
}

impl<F, M> EventLoopSender<F, M> {
    fn new(sender: Sender<EventLoopMessage<F, M>>) -> Self {
        Self { sender }
    }

    pub(crate) async fn send<T: Into<EventLoopMessage<F, M>>>(
        &mut self,
        m: T,
    ) -> Result<(), RsbtError> {
        self.sender.send(m.into()).await?;

        Ok(())
    }
}

impl<F, M> From<Sender<EventLoopMessage<F, M>>> for EventLoopSender<F, M> {
    fn from(sender: Sender<EventLoopMessage<F, M>>) -> Self {
        Self::new(sender)
    }
}
