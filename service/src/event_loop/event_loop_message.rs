use crate::RsbtError;
use tokio::sync::oneshot::Sender;

pub(crate) enum EventLoopMessage<M> {
    Start(Sender<Result<(), RsbtError>>),
    Stop(Sender<Result<(), RsbtError>>),
    Quit(Sender<Result<(), RsbtError>>),
    Loop(M),
}

impl<M> From<M> for EventLoopMessage<M> {
    fn from(m: M) -> Self {
        Self::Loop(m)
    }
}
