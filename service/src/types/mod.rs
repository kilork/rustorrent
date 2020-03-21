use std::convert::{TryFrom, TryInto};

use crate::errors::{RsbtError, TryFromBencode};

mod config;
#[macro_use]
mod bencode;
pub mod info;
pub mod message;
pub mod peer;
pub mod properties;
pub mod torrent;
pub mod udp_tracker;

pub use bencode::{BencodeBlob, BencodeValue};
pub use config::{Config, Settings};
pub use properties::Properties;

pub(crate) const HANDSHAKE_PREFIX: [u8; 28] =
    *b"\x13BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00";
