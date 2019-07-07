use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha1::{Digest, Sha1};

use super::*;

use crate::types::info::{TorrentInfo, TorrentInfoRaw};
use crate::types::peer::Peer;
use crate::SHA1_SIZE;

#[derive(Debug, PartialEq)]
pub struct Torrent {
    pub raw: Vec<u8>,
    pub announce_url: String,
    pub announce_list: Option<Vec<Vec<String>>>,
    pub creation_date: Option<i64>,
    pub info: BencodeBlob,
}

#[derive(Debug, PartialEq)]
pub struct TrackerAnnounce {
    pub interval: i64,
    pub peers: Vec<Peer>,
}

impl Torrent {
    pub fn info_sha1_hash(&self) -> [u8; SHA1_SIZE] {
        Sha1::digest(self.info.source.as_slice())[..]
            .try_into()
            .expect("20 bytes array expected from Sha1 calculation")
    }

    pub fn info(&self) -> Result<TorrentInfo, RustorrentError> {
        self.info
            .clone()
            .try_into()
            .map(|x: TorrentInfoRaw| x.into())
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

try_from_bencode!(TrackerAnnounce,
    normal: (
        "interval" => interval,
        "peers" => peers
    ),
    failure: "failure reason"
);

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
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn parse_torrent() {
        let torrent_bytes = b"d8:announce36:http://bt1.archive.org:6969/announce13:announce-listll36:http://bt1.archive.org:6969/announceel36:http://bt2.archive.org:6969/announceee4:infoi1ee";
        let _torrent: Torrent = torrent_bytes.to_vec().try_into().unwrap();
    }

    #[test]
    fn parse_peer() {
        let peer_bytes = b"d2:ip9:127.0.0.17:peer id20:rustorrent          4:porti6970ee";
        let peer: Peer = peer_bytes.to_vec().try_into().unwrap();
        assert_eq!(
            peer,
            Peer {
                ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                port: 6970,
                peer_id: Some("rustorrent          ".into())
            }
        );
    }

    #[test]
    fn parse_compact_0() {
        let tracker_response = b"d8:completei1e10:incompletei1e8:intervali600e5:peersld2:ip9:127.0.0.17:peer id20:-TR2940-pm2sh9i76t4d4:porti62437eed2:ip9:127.0.0.17:peer id20:rustorrent          4:porti6970eeee";
        let tracker_announce_response: TrackerAnnounce =
            tracker_response.to_vec().try_into().unwrap();
        assert_eq!(
            tracker_announce_response,
            TrackerAnnounce {
                interval: 600,
                peers: vec![
                    Peer {
                        ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        port: 62437,
                        peer_id: Some("-TR2940-pm2sh9i76t4d".into())
                    },
                    Peer {
                        ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        port: 6970,
                        peer_id: Some("rustorrent          ".into())
                    }
                ],
            }
        );
    }

    #[test]
    fn parse_compact_1() {
        let tracker_response = b"d8:completei1e10:incompletei1e8:intervali600e5:peers12:\x7F\x00\x00\x01\x1B:\x7F\x00\x00\x01\xF3\xE56:peers60:e";
        let tracker_announce_response: TrackerAnnounce =
            tracker_response.to_vec().try_into().unwrap();
        assert_eq!(
            tracker_announce_response,
            TrackerAnnounce {
                interval: 600,
                peers: vec![
                    Peer {
                        ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        port: 6970,
                        peer_id: None
                    },
                    Peer {
                        ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                        port: 62437,
                        peer_id: None
                    }
                ],
            }
        );
    }

    #[test]
    fn parse_announce_with_failure() {
        let tracker_response = b"d14:failure reason63:Requested download is not authorized for use with this tracker.e";
        let tracker_announce_response: Result<TrackerAnnounce, RustorrentError> =
            tracker_response.to_vec().try_into();
        match tracker_announce_response {
            Err(RustorrentError::FailureReason(failure_reason)) => assert_eq!(
                failure_reason,
                "Requested download is not authorized for use with this tracker."
            ),
            res => panic!("Unexpected result: {:?}", res),
        }
    }

    #[test]
    fn parse_announce_with_ipv6() {
        let tracker_response = b"d8:completei0e10:incompletei1e8:intervali600e5:peersld2:ip3:::17:peer id20:-rs0001-zzzzxxxxyyyy4:porti6881eeee";
        let tracker_announce_response: TrackerAnnounce =
            tracker_response.to_vec().try_into().unwrap();
        assert_eq!(
            tracker_announce_response,
            TrackerAnnounce {
                interval: 600,
                peers: vec![Peer {
                    ip: IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
                    port: 6881,
                    peer_id: Some("-rs0001-zzzzxxxxyyyy".into()),
                },],
            }
        );
    }
}
