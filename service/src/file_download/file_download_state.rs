use crate::RsbtError;
use futures::future::BoxFuture;
use std::fmt::{Debug, Formatter};
use tokio::sync::oneshot;

pub(crate) enum FileDownloadState {
    Idle,
    SendQueryPiece(
        BoxFuture<'static, Result<(), RsbtError>>,
        Option<oneshot::Receiver<Result<Vec<u8>, RsbtError>>>,
    ),
    ReceiveQueryPiece(oneshot::Receiver<Result<Vec<u8>, RsbtError>>),
}

impl Debug for FileDownloadState {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            FileDownloadState::Idle => write!(f, "Idle"),
            FileDownloadState::SendQueryPiece(_, _) => write!(f, "SendQueryPiece"),
            FileDownloadState::ReceiveQueryPiece(_) => write!(f, "ReceiveQueryPiece"),
        }
    }
}
