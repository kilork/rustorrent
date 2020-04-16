use crate::SHA1_SIZE;
use crate::{
    types::{
        info::{TorrentInfo, TorrentInfoRaw},
        BencodeBlob,
    },
    RsbtError,
};
use sha1::{Digest, Sha1};
use std::convert::{TryFrom, TryInto};

#[derive(Debug, PartialEq)]
pub struct Torrent {
    pub raw: Vec<u8>,
    pub announce_url: String,
    pub announce_list: Option<Vec<Vec<String>>>,
    pub creation_date: Option<i64>,
    pub info: BencodeBlob,
}

impl Torrent {
    pub fn info_sha1_hash(&self) -> [u8; SHA1_SIZE] {
        Sha1::digest(self.info.source.as_slice())[..]
            .try_into()
            .expect("20 bytes array expected from Sha1 calculation")
    }

    pub fn info(&self) -> Result<TorrentInfo, RsbtError> {
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
