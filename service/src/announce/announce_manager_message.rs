pub(crate) enum AnnounceManagerMessage {
    QueryAnnounce { tier: usize, tracker: usize },
    ProcessAnnounce,
}
