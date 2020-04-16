use crate::types::udp_tracker::{UdpTrackerRequest, UdpTrackerResponse};

/// Bittorrent UDP-tracker protocol extension.
///
/// A tracker with the protocol "udp://" in its URI is supposed to be contacted using this protocol.
///
/// Credits: Protocol designed by Olaf van der Spek and extended by Arvid Norberg.
///
/// Reference: https://www.libtorrent.org/udp_tracker_protocol.html
pub(crate) enum UdpTracker {
    Request(UdpTrackerRequest),
    Response(UdpTrackerResponse),
}
