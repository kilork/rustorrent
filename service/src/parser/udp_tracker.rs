use crate::types::{Peer, UdpTrackerResponse, UdpTrackerResponseData, UdpTrackerScrape};
use nom::combinator::map;
use nom::*;
use nom::{bytes::complete::take, number::complete::*};
use std::net::{IpAddr, Ipv4Addr};

pub(crate) fn parser_udp_tracker(i: &[u8]) -> IResult<&[u8], UdpTrackerResponse> {
    let (i, action) = be_i32(i)?;
    let (i, transaction_id) = be_i32(i)?;
    let (i, data) = match action {
        0 => map(be_i64, |connection_id| UdpTrackerResponseData::Connect {
            connection_id,
        })(i)?,
        1 => {
            let (i, interval) = be_i32(i)?;
            let (i, leechers) = be_i32(i)?;
            let (i, seeders) = be_i32(i)?;
            let (i, peers) = nom::multi::many0(peer)(i)?;
            (
                i,
                UdpTrackerResponseData::Announce {
                    interval,
                    leechers,
                    seeders,
                    peers,
                },
            )
        }
        2 => {
            let (i, info) = nom::multi::many0(scrape)(i)?;
            (i, UdpTrackerResponseData::Scrape { info })
        }
        3 => {
            let error_string = String::from_utf8_lossy(i).into();
            (
                &i[i.len()..],
                UdpTrackerResponseData::Error { error_string },
            )
        }
        other => {
            let error_string = format!("unknown udp tracker action: {}", other);
            (
                &i[i.len()..],
                UdpTrackerResponseData::Error { error_string },
            )
        }
    };
    Ok((
        i,
        UdpTrackerResponse {
            transaction_id,
            data,
        },
    ))
}

fn scrape(i: &[u8]) -> IResult<&[u8], UdpTrackerScrape> {
    let (i, complete) = be_i32(i)?;
    let (i, downloaded) = be_i32(i)?;
    let (i, incomplete) = be_i32(i)?;

    Ok((
        i,
        UdpTrackerScrape {
            complete,
            downloaded,
            incomplete,
        },
    ))
}

pub fn peer(i: &[u8]) -> IResult<&[u8], Peer> {
    let (i, peer) = take(4usize)(i)?;
    let (i, port) = be_u16(i)?;
    Ok((
        i,
        Peer {
            ip: IpAddr::V4(Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3])),
            port,
            peer_id: None,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(buf: &[u8], udp_tracker_response: UdpTrackerResponse) {
        assert_eq!(parser_udp_tracker(buf).unwrap().1, udp_tracker_response);
    }

    #[test]
    fn parse_udp_tracker_response_connect() {
        parse(
            &[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2],
            UdpTrackerResponse {
                data: UdpTrackerResponseData::Connect { connection_id: 2 },
                transaction_id: 1,
            },
        );
    }

    #[test]
    fn parse_udp_tracker_response_announce() {
        parse(
            &[
                0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0, 30, 1, 2, 3, 4, 0, 80,
            ],
            UdpTrackerResponse {
                data: UdpTrackerResponseData::Announce {
                    interval: 10,
                    leechers: 20,
                    seeders: 30,
                    peers: vec![Peer {
                        ip: IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)),
                        port: 80,
                        peer_id: None,
                    }],
                },
                transaction_id: 2,
            },
        );
    }
}
