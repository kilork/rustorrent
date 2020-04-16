#[derive(Debug, Clone)]
pub(crate) enum UdpTrackerRequestData {
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
