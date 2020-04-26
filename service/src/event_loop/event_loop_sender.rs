use crate::event_loop::EventLoopMessage;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

pub(crate) struct EventLoopSender<M> {
    inner: Arc<Sender<EventLoopMessage<M>>>,
}

impl<M> Clone for EventLoopSender<M> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<M> EventLoopSender<M> {
    fn new(sender: Sender<EventLoopMessage<M>>) -> Self {
        Self {
            inner: Arc::new(sender),
        }
    }
}

impl<M> From<Sender<EventLoopMessage<M>>> for EventLoopSender<M> {
    fn from(sender: Sender<EventLoopMessage<M>>) -> Self {
        Self::new(sender)
    }
}
