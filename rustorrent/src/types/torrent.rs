use super::*;
use crate::errors::RustorrentError;

use percent_encoding::{percent_encode, USERINFO_ENCODE_SET};
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
        let mut response = client
            .get(&format!(
                "{}?info_hash={}&peer_id={}",
                self.announce_url,
                url_encode(&info_hash[..]),
                url_encode(&PEER_ID[..])
            ))
            .send()?;

        let mut buf: Vec<u8> = vec![];
        response.copy_to(&mut buf)?;

        let bencode: BencodeBlob = buf[..].try_into()?;
        let bencode_dictionary: Vec<(_, _)> = bencode.value.try_into()?;

        dbg!(bencode_dictionary
            .iter()
            .map(|x| x.0)
            .collect::<Vec<&str>>());

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

impl<'a> TryFrom<BencodeBlob<'a>> for Torrent<'a> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeBlob<'a>) -> Result<Self, Self::Error> {
        let dictionary: Vec<_> = value.value.try_into()?;

        let mut announce_url = None;
        let mut announce_list = None;
        let mut creation_date = None;
        let mut info = None;

        for (key, value) in dictionary {
            match key {
                "announce" => announce_url = Some(value.value.try_into()?),
                "announce-list" => {
                    let value_list: Vec<BencodeBlob> = value.value.try_into()?;
                    let value_list_list: Vec<Vec<&str>> = value_list
                        .into_iter()
                        .map(|l| l.value.try_into().unwrap())
                        .map(|l: Vec<BencodeBlob>| {
                            l.into_iter().map(|k| k.value.try_into().unwrap()).collect()
                        })
                        .collect();
                    announce_list = Some(value_list_list);
                }
                "creation date" => creation_date = Some(value.value.try_into()?),
                "info" => info = Some(value),
                _ => (),
            }
        }

        Ok(Torrent {
            raw: value.source,
            announce_url: announce_url.unwrap(),
            announce_list,
            creation_date,
            info: info.unwrap(),
        })
    }
}
