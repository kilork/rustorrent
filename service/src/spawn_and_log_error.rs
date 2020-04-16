use crate::RsbtError;
use log::error;
use std::future::Future;

pub(crate) fn spawn_and_log_error<F, M>(f: F, message: M) -> tokio::task::JoinHandle<()>
where
    F: Future<Output = Result<(), RsbtError>> + Send + 'static,
    M: Fn() -> String + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = f.await {
            error!("{}: {}", message(), e)
        }
    })
}
