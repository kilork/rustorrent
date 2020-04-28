use crate::{event_loop::EventLoopMessage, RsbtError};
use tokio::sync::mpsc::Sender;

#[derive(Clone)]
pub(crate) struct EventLoopSender<M, F> {
    sender: Sender<EventLoopMessage<M>>,
    feedback_sender: Sender<F>,
}

impl<M, F> EventLoopSender<M, F> {
    pub(crate) fn new(sender: Sender<EventLoopMessage<M>>, feedback_sender: Sender<F>) -> Self {
        Self {
            sender,
            feedback_sender,
        }
    }

    pub(crate) async fn send<T: Into<EventLoopMessage<M>>>(
        &mut self,
        m: T,
    ) -> Result<(), RsbtError> {
        self.sender.send(m.into()).await?;

        Ok(())
    }

    pub(crate) async fn feedback(&mut self, f: F) -> Result<(), RsbtError> {
        self.feedback_sender.send(f).await?;

        Ok(())
    }
}
