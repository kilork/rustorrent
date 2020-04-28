use crate::RsbtError;
use tokio::sync::oneshot::Sender;

pub(crate) enum EventLoopMessage<M, F> {
    Start(Sender<Result<(), RsbtError>>),
    Stop(Sender<Result<(), RsbtError>>),
    Quit(Option<Sender<Result<(), RsbtError>>>),
    Feedback(F),
    Loop(M),
}

impl<M, F> From<M> for EventLoopMessage<M, F> {
    fn from(m: M) -> Self {
        Self::Loop(m)
    }
}
