use crate::types::Peer;
use tokio::time::Duration;

pub(crate) struct Announcement {
    pub(crate) requery_interval: Duration,
    pub(crate) peers: Vec<Peer>,
}
