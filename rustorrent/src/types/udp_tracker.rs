use super::*;
use crate::{app::TorrentProcess, parser::parser_udp_tracker};
use bytes::{Buf, BufMut, BytesMut};
use failure::Fail;
use nom::Offset;
use rand::prelude::*;
use std::{
    fmt::{Display, Formatter},
    sync::Arc,
};
use tokio_util::codec::{Decoder, Encoder};

/// Bittorrent UDP-tracker protocol extension.
///
/// A tracker with the protocol "udp://" in its URI is supposed to be contacted using this protocol.
///
/// Credits: Protocol designed by Olaf van der Spek and extended by Arvid Norberg.
///
/// Reference: https://www.libtorrent.org/udp_tracker_protocol.html
pub(crate) enum UdpTracker {
    Request(UdpTrackerRequest),
    Response(UdpTrackerResponse),
}

#[derive(Debug, Clone)]
pub(crate) struct UdpTrackerRequest {
    /// Must be initialized to 0x41727101980 in network byte order for connect.
    /// This will identify the protocol.
    pub(crate) connection_id: i64,
    pub(crate) transaction_id: i32,
    pub(crate) data: UdpTrackerRequestData,
    pub(crate) authentication: Option<UdpTrackerAuthentication>,
    pub(crate) request_string: Option<String>,
}

impl UdpTrackerRequest {
    pub(crate) fn connect() -> Self {
        Self {
            connection_id: 0x41727101980,
            transaction_id: random(),
            data: UdpTrackerRequestData::Connect,
            authentication: None,
            request_string: None,
        }
    }

    pub(crate) fn announce(
        connection_id: i64,
        settings: Arc<Settings>,
        torrent_process: Arc<TorrentProcess>,
    ) -> Self {
        let left = torrent_process.info.len() as i64;

        Self {
            connection_id,
            transaction_id: random(),
            data: UdpTrackerRequestData::Announce {
                info_hash: torrent_process.hash_id,
                peer_id: crate::PEER_ID,
                downloaded: 0,
                uploaded: 0,
                left,
                event: 0,
                ip: 0,
                extensions: 0,
                num_want: -1,
                key: random(),
                port: settings.config.port,
            },
            authentication: None,
            request_string: None,
        }
    }

    pub(crate) fn match_response(&self, response: &UdpTrackerResponse) -> bool {
        match (self, response) {
            (
                UdpTrackerRequest {
                    transaction_id: request_transaction_id,
                    data: request_data,
                    ..
                },
                UdpTrackerResponse {
                    transaction_id: response_transaction_id,
                    data: response_data,
                    ..
                },
            ) if request_transaction_id == response_transaction_id => {
                match (request_data, response_data) {
                    (UdpTrackerRequestData::Connect, UdpTrackerResponseData::Connect { .. })
                    | (
                        UdpTrackerRequestData::Announce { .. },
                        UdpTrackerResponseData::Announce { .. },
                    )
                    | (
                        UdpTrackerRequestData::Scrape { .. },
                        UdpTrackerResponseData::Scrape { .. },
                    ) => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum UdpTrackerRequestData {
    /// connecting
    Connect,
    /// announcing
    Announce {
        /// The info-hash of the torrent you want announce yourself in.
        info_hash: [u8; 20],
        /// Your peer id.
        peer_id: [u8; 20],
        /// The number of byte you've downloaded in this session.
        downloaded: i64,
        /// The number of bytes you have left to download until you're finished.
        left: i64,
        /// The number of bytes you have uploaded in this session.
        uploaded: i64,
        /// The event, one of
        /// none = 0
        /// completed = 1
        /// started = 2
        /// stopped = 3
        event: i32,
        /// Your ip address. Set to 0 if you want the tracker to use the sender of this UDP packet.
        ip: u32,
        /// A unique key that is randomized by the client.
        key: u32,
        /// The maximum number of peers you want in the reply. Use -1 for default.
        num_want: i32,
        /// The port you're listening on.
        port: u16,
        /// See extensions.
        extensions: u16,
    },
    /// scraping
    Scrape { info_hashes: Vec<[u8; 20]> },
}

#[derive(Debug, PartialEq)]
pub(crate) struct UdpTrackerResponse {
    pub(crate) transaction_id: i32,
    pub(crate) data: UdpTrackerResponseData,
}

#[derive(Debug, PartialEq)]
pub(crate) enum UdpTrackerResponseData {
    /// connecting
    Connect {
        /// A connection id, this is used when further information is exchanged with
        /// the tracker, to identify you. This connection id can be reused for multiple
        /// requests, but if it's cached for too long, it will not be valid anymore.
        connection_id: i64,
    },
    /// announcing
    Announce {
        /// The number of seconds you should wait until re-announcing yourself.
        interval: i32,
        /// The number of peers in the swarm that has not finished downloading.
        leechers: i32,
        /// The number of peers in the swarm that has finished downloading and are seeding.
        seeders: i32,
        /// The rest of the server reply is a variable number of the following structure:
        /// int32_t | ip | The ip of a peer in the swarm.
        /// uint16_t | port | The peer's listen port.
        peers: Vec<crate::types::peer::Peer>,
    },
    /// scraping
    Scrape {
        info: Vec<UdpTrackerScrape>,
    },
    Error {
        error_string: String,
    },
}

#[derive(Debug, PartialEq)]
pub(crate) struct UdpTrackerScrape {
    pub(crate) complete: i32,
    pub(crate) downloaded: i32,
    pub(crate) incomplete: i32,
}

#[derive(Debug, Clone)]
pub(crate) struct UdpTrackerAuthentication {
    /// User name.
    username: String,
    /// Password.
    /// Would be send as sha1(packet + sha1(password)) The packet in this case means
    /// the entire packet except these 8 bytes that are the password hash.
    /// These are the 8 first bytes (most significant) from the 20 bytes hash calculated.
    password: String,
}

#[derive(Fail, Debug)]
pub enum UdpTrackerCodecError {
    // #[fail(display = "Channel Error: {}", _0)]
    // ChannelError(tokio::sync::mpsc::error::UnboundedRecvError),
    #[fail(display = "IO Error: {}", _0)]
    IoError(std::io::Error),
    #[fail(display = "Couldn't parse incoming frame: {}", _0)]
    ParseError(String),
}

impl From<std::io::Error> for UdpTrackerCodecError {
    fn from(err: std::io::Error) -> Self {
        UdpTrackerCodecError::IoError(err)
    }
}

#[derive(Default)]
pub(crate) struct UdpTrackerCodec;

impl Decoder for UdpTrackerCodec {
    type Item = UdpTrackerResponse;
    type Error = UdpTrackerCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (consumed, f) = match parser_udp_tracker(buf) {
            Err(e) => {
                if e.is_incomplete() {
                    return Ok(None);
                } else {
                    return Err(UdpTrackerCodecError::ParseError(format!("{:?}", e)));
                }
            }
            Ok((i, frame)) => (buf.offset(i), frame),
        };

        buf.advance(consumed);

        Ok(Some(f))
    }
}

impl Encoder for UdpTrackerCodec {
    type Item = UdpTrackerRequest;
    type Error = UdpTrackerCodecError;

    fn encode(&mut self, frame: Self::Item, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let UdpTrackerRequest {
            connection_id,
            transaction_id,
            data,
            authentication,
            request_string,
        } = frame;
        match data {
            UdpTrackerRequestData::Connect => {
                buf.reserve(16);
                buf.put_i64(connection_id);
                buf.put_i32(0);
                buf.put_i32(transaction_id);
            }
            UdpTrackerRequestData::Announce {
                info_hash,
                peer_id,
                downloaded,
                left,
                uploaded,
                event,
                ip,
                key,
                num_want,
                port,
                extensions,
            } => {
                buf.reserve(16 + 20 + 20 + 8 + 8 + 8 + 4 + 4 + 4 + 4 + 2 + 2);
                buf.put_i64(connection_id);
                buf.put_i32(1);
                buf.put_i32(transaction_id);
                buf.put(info_hash.as_ref());
                buf.put(peer_id.as_ref());
                buf.put_i64(downloaded);
                buf.put_i64(left);
                buf.put_i64(uploaded);
                buf.put_i32(event);
                buf.put_u32(ip);
                buf.put_u32(key);
                buf.put_i32(num_want);
                buf.put_u16(port);
                buf.put_u16(extensions);
            }
            UdpTrackerRequestData::Scrape { info_hashes } => {
                buf.reserve(16 + 20 * info_hashes.len());
                buf.put_i64(connection_id);
                buf.put_i32(2);
                buf.put_i32(transaction_id);
                for info_hash in info_hashes {
                    buf.put(info_hash.as_ref());
                }
            }
        }

        Ok(())
    }
}
