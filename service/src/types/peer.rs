use crate::{
    errors::TryFromBencode,
    types::{BencodeBlob, BencodeValue},
    RsbtError,
};
use std::{
    convert::{TryFrom, TryInto},
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

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

impl From<SocketAddr> for Peer {
    fn from(value: SocketAddr) -> Peer {
        Peer {
            ip: value.ip(),
            peer_id: None,
            port: value.port(),
        }
    }
}

impl Into<SocketAddr> for Peer {
    fn into(self) -> SocketAddr {
        SocketAddr::new(self.ip, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn parse_peer() {
        let peer_bytes = b"d2:ip9:127.0.0.17:peer id20:rsbt                4:porti6970ee";
        let peer: Peer = peer_bytes.to_vec().try_into().unwrap();
        assert_eq!(
            peer,
            Peer {
                ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                port: 6970,
                peer_id: Some("rsbt                ".into())
            }
        );
    }
}
