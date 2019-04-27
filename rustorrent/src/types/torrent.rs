use super::*;
use crate::errors::RustorrentError;

use log::debug;
use percent_encoding::{percent_encode, percent_encode_byte, SIMPLE_ENCODE_SET};
use reqwest;
use sha1::{
    digest::generic_array::{typenum::U20, GenericArray},
    Digest, Sha1,
};

use std::convert::TryInto;
use std::fs::File;
use std::io::Read;
use std::net::Ipv4Addr;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub struct Torrent {
    pub raw: Vec<u8>,
    pub announce_url: String,
    pub announce_list: Option<Vec<Vec<String>>>,
    pub creation_date: Option<i64>,
    pub info: BencodeBlob,
}

#[derive(Debug, PartialEq)]
pub struct TrackerAnnounceResponse {
    pub interval: Option<i64>,
    pub failure_reason: Option<String>,
    pub peers: Option<Vec<Peer>>,
}

#[derive(Debug, PartialEq)]
pub struct Peer {
    pub ip: Ipv4Addr,
    pub peer_id: Option<String>,
    pub port: u16,
}

impl Torrent {
    /*
        pub fn announce(
            &self,
            settings: &Settings,
        ) -> Result<TrackerAnnounceResponse, RustorrentError> {
            let info_hash = self.info_sha1_hash();

            let client = reqwest::r#async::Client::new();

            let mut url = format!(
                "{}?info_hash={}&peer_id={}",
                self.announce_url,
                url_encode(&info_hash[..]),
                url_encode(&PEER_ID[..])
            );

            let config = &settings.config;

            if let Some(port) = config.port {
                url += format!("&port={}", port).as_str();
            }

            if let Some(compact) = config.compact {
                url += format!("&compact={}", if compact { 1 } else { 0 }).as_str();
            }

            debug!("Get tracker announce from: {}", url);

            let mut response = client.get(&url).send()?;

            let mut buf = vec![];
            response.copy_to(&mut buf)?;

            debug!(
                "Tracker response (url encoded): {}",
                percent_encode(&buf, SIMPLE_ENCODE_SET).to_string()
            );
            let tracker_announce_response = buf.try_into()?;
            debug!("Tracker response parsed: {:#?}", tracker_announce_response);

            Ok(tracker_announce_response)
        }
    */
    pub fn info_sha1_hash(&self) -> GenericArray<u8, U20> {
        Sha1::digest(self.info.source.as_slice())
    }
}

try_from_bencode!(Torrent,
    normal: ("announce" => announce_url),
    optional: (
        "announce-list" => announce_list,
        "creation date" => creation_date
    ),
    bencode: ("info" => info),
    raw: (raw)
);

try_from_bencode!(TrackerAnnounceResponse,
    optional: (
        "interval" => interval,
        "failure reason" => failure_reason,
        "peers" => peers
    )
);

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

pub fn parse_torrent(filename: impl AsRef<Path>) -> Result<Torrent, RustorrentError> {
    let mut buf = vec![];

    let mut f = File::open(filename)?;

    f.read_to_end(&mut buf)?;

    let torrent = buf.try_into()?;

    Ok(torrent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_peer() {
        let peer_bytes = b"d2:ip9:127.0.0.17:peer id20:rustorrent          4:porti6970ee";
        let peer: Peer = peer_bytes.to_vec().try_into().unwrap();
        assert_eq!(
            peer,
            Peer {
                ip: Ipv4Addr::new(127, 0, 0, 1),
                port: 6970,
                peer_id: Some("rustorrent          ".into())
            }
        );
    }

    #[test]
    fn parse_compact_0() {
        let tracker_response = b"d8:completei1e10:incompletei1e8:intervali600e5:peersld2:ip9:127.0.0.17:peer id20:-TR2940-pm2sh9i76t4d4:porti62437eed2:ip9:127.0.0.17:peer id20:rustorrent          4:porti6970eeee";
        let tracker_announce_response: TrackerAnnounceResponse =
            tracker_response.to_vec().try_into().unwrap();
        assert_eq!(
            tracker_announce_response,
            TrackerAnnounceResponse {
                interval: Some(600),
                failure_reason: None,
                peers: Some(vec![
                    Peer {
                        ip: Ipv4Addr::new(127, 0, 0, 1),
                        port: 62437,
                        peer_id: Some("-TR2940-pm2sh9i76t4d".into())
                    },
                    Peer {
                        ip: Ipv4Addr::new(127, 0, 0, 1),
                        port: 6970,
                        peer_id: Some("rustorrent          ".into())
                    }
                ]),
            }
        );
    }

    #[test]
    fn parse_compact_1() {
        let tracker_response = b"d8:completei1e10:incompletei1e8:intervali600e5:peers12:\x7F\x00\x00\x01\x1B:\x7F\x00\x00\x01\xF3\xE56:peers60:e";
        let tracker_announce_response: TrackerAnnounceResponse =
            tracker_response.to_vec().try_into().unwrap();
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
