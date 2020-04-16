use crate::types::udp_tracker::UdpTrackerScrape;

#[derive(Debug, PartialEq)]
pub(crate) enum UdpTrackerResponseData {
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
