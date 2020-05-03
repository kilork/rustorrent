use crate::RsbtError;
use tokio::time::Duration;

pub(crate) enum AnnounceManagerMessage {
    QueryAnnounce { tier: usize, tracker: usize, delay: Option<Duration> },
    QueryAnnounceResult(Result<(), RsbtError>),
}
