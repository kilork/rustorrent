use crate::{
    event_loop::{EventLoopCommand, EventLoopMessage},
    RsbtError,
};
use std::future::Future;
use tokio::sync::mpsc::Sender;

pub(crate) struct EventLoopSender<M, F> {
    sender: Sender<EventLoopMessage<M>>,
    feedback_sender: Sender<F>,
}

impl<M, F> Clone for EventLoopSender<M, F> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            feedback_sender: self.feedback_sender.clone(),
        }
    }
}

impl<M: Send + 'static, F> EventLoopSender<M, F> {
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

    pub(crate) fn command<FF, R, MF>(&self, f: FF, mf: MF) -> EventLoopCommand
    where
        FF: Future<Output = Result<R, RsbtError>> + Send + 'static,
        MF: FnOnce(Result<R, RsbtError>) -> M + Send + 'static,
        R: Send + 'static,
    {
        EventLoopCommand::command(f, self.sender.clone(), mf)
    }
}
