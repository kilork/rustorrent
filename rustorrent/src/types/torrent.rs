use super::*;
use crate::errors::RustorrentError;

use log::debug;
use percent_encoding::{percent_encode, SIMPLE_ENCODE_SET, USERINFO_ENCODE_SET};
use reqwest;
use sha1::{Digest, Sha1};

use std::convert::TryInto;

#[derive(Debug)]
pub struct Torrent<'a> {
    pub raw: &'a [u8],
    pub announce_url: &'a str,
    pub announce_list: Option<Vec<Vec<&'a str>>>,
    pub creation_date: Option<i64>,
    pub info: BencodeBlob<'a>,
}

#[derive(Debug)]
pub struct TrackerAnnounceResponse<'a> {
    pub interval: Option<i64>,
    pub failure_reason: Option<&'a str>,
    // pub peers: Option<Vec<Peer<'a>>>,
}

#[derive(Debug)]
pub struct Peer<'a> {
    pub id: Option<&'a str>,
}

const PEER_ID: [u8; 20] = *b"rustorrent          ";

fn url_encode(data: &[u8]) -> String {
    percent_encode(data, USERINFO_ENCODE_SET).to_string()
}

impl<'a> Torrent<'a> {
    pub fn announce(&self) -> Result<(), RustorrentError> {
        let mut hasher = Sha1::default();
        hasher.input(self.info.source);
        let info_hash = hasher.result();

        let client = reqwest::Client::new();

        let url = format!(
            "{}?info_hash={}&peer_id={}",
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

        let bencode: BencodeBlob = buf[..].try_into()?;

        let tracker_announce_response: TrackerAnnounceResponse = bencode.try_into()?;
        dbg!(tracker_announce_response);

        Ok(())
    }
}

impl<'a> TryFrom<&'a [u8]> for Torrent<'a> {
    type Error = RustorrentError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        let bencode: BencodeBlob = value.try_into()?;
        bencode.try_into().map_err(RustorrentError::from)
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
        "failure reason" => failure_reason
        // "peers" => peers
    ),
);
