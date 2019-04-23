use super::*;
use crate::errors::RustorrentError;
use std::net::Ipv4Addr;

use log::debug;
use percent_encoding::{percent_encode, percent_encode_byte, SIMPLE_ENCODE_SET};
use reqwest;
use sha1::{Digest, Sha1};

use std::convert::TryInto;

#[derive(Debug, PartialEq)]
pub struct Torrent<'a> {
    pub raw: &'a [u8],
    pub announce_url: &'a str,
    pub announce_list: Option<Vec<Vec<&'a str>>>,
    pub creation_date: Option<i64>,
    pub info: BencodeBlob<'a>,
}

#[derive(Debug, PartialEq)]
pub struct TrackerAnnounceResponse<'a> {
    pub interval: Option<i64>,
    pub failure_reason: Option<&'a str>,
    pub peers: Option<Vec<Peer<'a>>>,
}

#[derive(Debug, PartialEq)]
pub struct Peer<'a> {
    pub ip: Ipv4Addr,
    pub peer_id: Option<&'a str>,
    pub port: u16,
}

const PEER_ID: [u8; 20] = *b"rustorrent          ";

fn url_encode(data: &[u8]) -> String {
    data.iter()
        .map(|&x| percent_encode_byte(x))
        .collect::<String>()
}

impl<'a> Torrent<'a> {
    pub fn announce(&self) -> Result<(), RustorrentError> {
        let mut hasher = Sha1::default();
        hasher.input(self.info.source);
        let info_hash = hasher.result();

        let client = reqwest::Client::new();

        let url = format!(
            "{}?info_hash={}&peer_id={}&port=6970&compact=1",
            self.announce_url,
            url_encode(&info_hash[..]),
            url_encode(&PEER_ID[..])
        );

        debug!("Get tracker announce from: {}", url);

        let mut response = client.get(&url).send()?;

        let mut buf: Vec<u8> = vec![];
        response.copy_to(&mut buf)?;

        debug!(
            "Tracker response (url encoded): {}",
            percent_encode(&buf, SIMPLE_ENCODE_SET).to_string()
        );

        let tracker_announce_response: TrackerAnnounceResponse = buf.as_slice().try_into()?;

        dbg!(tracker_announce_response);

        Ok(())
    }
}

try_from_bencode!(Torrent<'a>,
    normal: ("announce" => announce_url),
    optional: (
        "announce-list" => announce_list,
        "creation date" => creation_date
    ),
    bencode: ("info" => info),
    raw: (raw)
);

try_from_bencode!(TrackerAnnounceResponse<'a>,
    optional: (
        "interval" => interval,
        "failure reason" => failure_reason,
        "peers" => peers
    )
);

try_from_bencode!(Peer<'a>,
    normal: (
        "ip" => ip,
        "port" => port
    ),
    optional: (
        "peer id" => peer_id
    )
);

impl<'a> TryFrom<BencodeBlob<'a>> for Vec<Peer<'a>> {
    type Error = TryFromBencode;

    fn try_from(blob: BencodeBlob<'a>) -> Result<Self, Self::Error> {
        match blob.value {
            BencodeValue::String(s) => Ok(s
                .chunks_exact(6)
                .map(|peer| Peer {
                    ip: Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3]),
                    port: u16::from(peer[4]) * 256u16 + u16::from(peer[5]),
                    peer_id: None,
                })
                .collect()),
            BencodeValue::List(l) => Ok(l.into_iter().map(|x| x.try_into().unwrap()).collect()),
            _ => Err(TryFromBencode::NotDictionary),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_peer() {
        let peer_bytes = b"d2:ip9:127.0.0.17:peer id20:rustorrent          4:porti6970ee";
        let peer: Peer = peer_bytes[..].try_into().unwrap();
        assert_eq!(
            peer,
            Peer {
                ip: Ipv4Addr::new(127, 0, 0, 1),
                port: 6970,
                peer_id: Some("rustorrent          ")
            }
        );
    }

    #[test]
    fn parse_compact_0() {
        let tracker_response = b"d8:completei1e10:incompletei1e8:intervali600e5:peersld2:ip9:127.0.0.17:peer id20:-TR2940-pm2sh9i76t4d4:porti62437eed2:ip9:127.0.0.17:peer id20:rustorrent          4:porti6970eeee";
        let tracker_announce_response: TrackerAnnounceResponse =
            tracker_response[..].try_into().unwrap();
        assert_eq!(
            tracker_announce_response,
            TrackerAnnounceResponse {
                interval: Some(600),
                failure_reason: None,
                peers: Some(vec![
                    Peer {
                        ip: Ipv4Addr::new(127, 0, 0, 1),
                        port: 62437,
                        peer_id: Some("-TR2940-pm2sh9i76t4d")
                    },
                    Peer {
                        ip: Ipv4Addr::new(127, 0, 0, 1),
                        port: 6970,
                        peer_id: Some("rustorrent          ")
                    }
                ]),
            }
        );
    }

    #[test]
    fn parse_compact_1() {
        let tracker_response = b"d8:completei1e10:incompletei1e8:intervali600e5:peers12:\x7F\x00\x00\x01\x1B:\x7F\x00\x00\x01\xF3\xE56:peers60:e";
        let tracker_announce_response: TrackerAnnounceResponse =
            tracker_response[..].try_into().unwrap();
        assert_eq!(
            tracker_announce_response,
            TrackerAnnounceResponse {
                interval: Some(600),
                failure_reason: None,
                peers: Some(vec![
                    Peer {
                        ip: Ipv4Addr::new(127, 0, 0, 1),
                        port: 6970,
                        peer_id: None
                    },
                    Peer {
                        ip: Ipv4Addr::new(127, 0, 0, 1),
                        port: 62437,
                        peer_id: None
                    }
                ]),
            }
        );
    }
}
