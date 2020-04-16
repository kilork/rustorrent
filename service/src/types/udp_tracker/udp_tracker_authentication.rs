#[derive(Debug, Clone)]
pub(crate) struct UdpTrackerAuthentication {
    /// User name.
    username: String,
    /// Password.
    /// Would be send as sha1(packet + sha1(password)) The packet in this case means
    /// the entire packet except these 8 bytes that are the password hash.
    /// These are the 8 first bytes (most significant) from the 20 bytes hash calculated.
    password: String,
}
