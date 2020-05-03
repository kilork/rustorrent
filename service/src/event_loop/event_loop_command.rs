use crate::RsbtError;
use futures::future::{abortable, AbortHandle};
use log::error;
use std::future::Future;
use tokio::sync::mpsc::Sender;

pub(crate) struct EventLoopCommand {
    abort_handle: AbortHandle,
}

impl EventLoopCommand {
    pub(crate) fn spawn<F>(f: F) -> Self
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let (task, abort_handle) = abortable(f);
        tokio::spawn(task);
        Self { abort_handle }
    }

    pub(crate) fn command<F, R, MF, C, K>(f: F, mut sender: Sender<C>, mf: MF) -> Self
    where
        F: Future<Output = Result<R, RsbtError>> + Send + 'static,
        MF: FnOnce(Result<R, RsbtError>) -> K + Send + 'static,
        K: Into<C> + Send + 'static,
        C: Send + 'static,
        R: Send + 'static,
    {
        Self::spawn(async move {
            let result = f.await;
            if let Err(err) = sender.send(mf(result).into()).await {
                error!("cannot send event loop command: {}", err);
            }
        })
    }

    pub(crate) fn abort(&self) {
        self.abort_handle.abort();
    }
}
