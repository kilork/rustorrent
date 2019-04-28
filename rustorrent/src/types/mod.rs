use std::convert::{TryFrom, TryInto};

use crate::errors::{RustorrentError, TryFromBencode};

mod config;
#[macro_use]
mod bencode;
pub mod peer;
pub mod torrent;
pub mod message;

pub use bencode::{BencodeBlob, BencodeValue};
pub use config::{Config, Settings};

pub(crate) const HANDSHAKE_PREFIX: [u8; 28] =
    *b"\x13BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00";
