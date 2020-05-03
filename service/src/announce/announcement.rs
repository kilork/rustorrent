use crate::types::Peer;
use std::time::Duration;

pub(crate) struct Announcement {
    pub(crate) requery_interval: Duration,
    pub(crate) peers: Vec<Peer>,
}
