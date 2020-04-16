use crate::types::peer::Peer;
use crate::{types::BencodeBlob, RsbtError};
use std::convert::{TryFrom, TryInto};

#[derive(Debug, PartialEq)]
pub struct TrackerAnnounce {
    /// Interval to reannounce in seconds
    pub interval: i64,
    pub peers: Vec<Peer>,
}

try_from_bencode!(TrackerAnnounce,
    normal: (
        "interval" => interval,
        "peers" => peers
    ),
    failure: "failure reason"
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn parse_compact_0() {
        let tracker_response = b"d8:completei1e10:incompletei1e8:intervali600e5:peersld2:ip9:127.0.0.17:peer id20:-TR2940-pm2sh9i76t4d4:porti62437eed2:ip9:127.0.0.17:peer id20:rsbt                4:porti6970eeee";
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
                        peer_id: Some("rsbt                ".into())
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
        let tracker_response = b"d14:failure reason63:Requested download is not authorized for use with this tracker.e".to_vec();
        let tracker_announce_response: Result<TrackerAnnounce, RsbtError> =
            tracker_response.try_into();
        match tracker_announce_response {
            Err(RsbtError::FailureReason(failure_reason)) => assert_eq!(
                failure_reason,
                "Requested download is not authorized for use with this tracker."
            ),
            res => panic!("Unexpected result: {:?}", res),
        }
    }

    #[test]
    fn parse_announce_with_ipv6() {
        let tracker_response = b"d8:completei0e10:incompletei1e8:intervali600e5:peersld2:ip3:::17:peer id20:-rs0001-zzzzxxxxyyyy4:porti6881eeee".to_vec();
        let tracker_announce_response: TrackerAnnounce = tracker_response.try_into().unwrap();
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
