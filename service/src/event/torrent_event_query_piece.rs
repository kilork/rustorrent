use std::{
    sync::{Arc, Mutex},
    task::Waker,
};

#[derive(Debug)]
pub(crate) struct TorrentEventQueryPiece {
    pub(crate) piece: usize,
    pub(crate) waker: Arc<Mutex<Option<Waker>>>,
}
