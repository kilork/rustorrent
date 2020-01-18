use super::*;

/// Bittorrent UDP-tracker protocol extension.
///
/// A tracker with the protocol "udp://" in its URI is supposed to be contacted using this protocol.
///
/// Credits: Protocol designed by Olaf van der Spek and extended by Arvid Norberg.
///
/// Reference: https://www.libtorrent.org/udp_tracker_protocol.html
pub(crate) enum UdpTracker {
    Request {
        /// Must be initialized to 0x41727101980 in network byte order for connect.
        /// This will identify the protocol.
        connection_id: i64,
        transaction_id: i32,
        data: UdpTrackerRequest,
        authentication: Option<UdpTrackerAuthentication>,
        request_string: Option<String>,
    },
    Response {
        transaction_id: i32,
        data: UdpTrackerResponse,
    },
}

pub(crate) enum UdpTrackerRequest {
    /// connecting
    Connect,
    /// announcing
    Announce {
        /// The info-hash of the torrent you want announce yourself in.
        info_hash: [u8; 20],
        /// Your peer id.
        peer_id: [u8; 20],
        /// The number of byte you've downloaded in this session.
        downloaded: i64,
        /// The number of bytes you have left to download until you're finished.
        left: i64,
        /// The number of bytes you have uploaded in this session.
        uploaded: i64,
        /// The event, one of
        /// none = 0
        /// completed = 1
        /// started = 2
        /// stopped = 3
        event: i32,
        /// Your ip address. Set to 0 if you want the tracker to use the sender of this UDP packet.
        ip: u32,
        /// A unique key that is randomized by the client.
        key: u32,
        /// The maximum number of peers you want in the reply. Use -1 for default.
        num_want: i32,
        /// The port you're listening on.
        port: u16,
        /// See extensions.
        extensions: u16,
    },
    /// scraping
    Scrape { info_hashes: Vec<[u8; 20]> },
}

pub(crate) enum UdpTrackerResponse {
    /// connecting
    Connect {
        /// A connection id, this is used when further information is exchanged with
        /// the tracker, to identify you. This connection id can be reused for multiple
        /// requests, but if it's cached for too long, it will not be valid anymore.
        connection_id: i64,
    },
    /// announcing
    Announce {
        /// The number of seconds you should wait until re-announcing yourself.
        interval: i32,
        /// The number of peers in the swarm that has not finished downloading.
        leechers: i32,
        /// The number of peers in the swarm that has finished downloading and are seeding.
        seeders: i32,
        /// The rest of the server reply is a variable number of the following structure:
        /// int32_t | ip | The ip of a peer in the swarm.
        /// uint16_t | port | The peer's listen port.
        peers: Vec<crate::types::peer::Peer>,
    },
    /// scraping
    Scrape {
        info: Vec<UdpTrackerScrape>,
    },
    Error {
        error_string: String,
    },
}

pub(crate) struct UdpTrackerScrape {
    complete: i32,
    downloaded: i32,
    incomplete: i32,
}

pub(crate) struct UdpTrackerAuthentication {
    /// User name.
    username: String,
    /// Password.
    /// Would be send as sha1(packet + sha1(password)) The packet in this case means
    /// the entire packet except these 8 bytes that are the password hash.
    /// These are the 8 first bytes (most significant) from the 20 bytes hash calculated.
    password: String,
}