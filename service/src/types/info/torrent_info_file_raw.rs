use crate::{
    errors::TryFromBencode,
    types::{BencodeBlob, BencodeValue},
    RsbtError,
};
use std::convert::{TryFrom, TryInto};

#[derive(Debug, PartialEq)]
pub struct TorrentInfoFileRaw {
    pub length: i64,
    pub path: Vec<String>,
}

try_from_bencode!(TorrentInfoFileRaw,
    normal: (
        "length" => length,
        "path" => path
    ),
);

impl TryFrom<BencodeBlob> for Vec<TorrentInfoFileRaw> {
    type Error = TryFromBencode;

    fn try_from(blob: BencodeBlob) -> Result<Self, Self::Error> {
        match blob.value {
            BencodeValue::List(l) => Ok(l.into_iter().map(|x| x.try_into().unwrap()).collect()),
            _ => Err(TryFromBencode::NotList),
        }
    }
}
