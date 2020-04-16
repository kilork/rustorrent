#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PeerState {
    Choked,
    Interested,
}
