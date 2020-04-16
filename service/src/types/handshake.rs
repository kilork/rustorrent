use crate::{parser::parse_handshake, RsbtError, SHA1_SIZE};
use std::convert::TryFrom;

#[derive(Debug)]
pub struct Handshake {
    pub protocol_prefix: [u8; 20],
    pub reserved: [u8; 8],
    pub info_hash: [u8; SHA1_SIZE],
    pub peer_id: [u8; 20],
}

impl TryFrom<Vec<u8>> for Handshake {
    type Error = RsbtError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        parse_handshake(&value)
    }
}
