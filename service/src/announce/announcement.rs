use crate::types::Peer;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct Announcement {
    pub(crate) announce_url: String,
    pub(crate) requery_interval: Duration,
    pub(crate) peers: Vec<Peer>,
}
