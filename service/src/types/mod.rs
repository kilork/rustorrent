#[macro_use]
mod bencode;
mod configuration;
mod handshake;
pub mod info;
mod message;
mod message_codec;
mod message_codec_error;
mod peer;
mod peer_state;
pub mod public;
mod torrent;
mod tracker_announce;
pub(crate) mod udp_tracker;

pub use bencode::{BencodeBlob, BencodeValue};
pub use configuration::{Config, Properties, Settings};
pub(crate) use handshake::Handshake;
pub use message::Message;
pub use message_codec::MessageCodec;
pub use message_codec_error::MessageCodecError;
pub use peer::Peer;
pub use torrent::Torrent;
pub(crate) use tracker_announce::TrackerAnnounce;
pub use udp_tracker::UdpTrackerCodecError;
pub(crate) use udp_tracker::{UdpTrackerResponse, UdpTrackerResponseData, UdpTrackerScrape};

pub(crate) const HANDSHAKE_PREFIX: [u8; 28] =
    *b"\x13BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00";
