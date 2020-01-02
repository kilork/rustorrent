use super::*;
use crate::parser::parse_handshake;

use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, PartialEq, Clone)]
pub struct Peer {
    pub ip: IpAddr,
    pub peer_id: Option<String>,
    pub port: u16,
}

try_from_bencode!(Peer,
    normal: (
        "ip" => ip,
        "port" => port
    ),
    optional: (
        "peer id" => peer_id
    )
);

impl TryFrom<BencodeBlob> for Vec<Peer> {
    type Error = TryFromBencode;

    fn try_from(blob: BencodeBlob) -> Result<Self, Self::Error> {
        match blob.value {
            BencodeValue::String(s) => Ok(s
                .chunks_exact(6)
                .map(|peer| Peer {
                    ip: IpAddr::V4(Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3])),
                    port: u16::from(peer[4]) * 256u16 + u16::from(peer[5]),
                    peer_id: None,
                })
                .collect()),
            BencodeValue::List(l) => Ok(l.into_iter().map(|x| x.try_into().unwrap()).collect()),
            _ => Err(TryFromBencode::NotDictionary),
        }
    }
}

#[derive(Debug)]
pub struct Handshake {
    pub protocol_prefix: [u8; 20],
    pub reserved: [u8; 8],
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl TryFrom<Vec<u8>> for Handshake {
    type Error = RustorrentError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        parse_handshake(&value)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PeerState {
    Choked,
    Interested,
}

pub struct PeerConnectionState {
    pub client_state: PeerState,
    pub peer_state: PeerState,
}
