use std::convert::TryFrom;
use std::ops::Deref;

use crate::errors::TryFromBencode;

mod bencode;
mod torrent;

pub use bencode::{BencodeBlob, BencodeValue};
pub use torrent::Torrent;
