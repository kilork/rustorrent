use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};

use crate::errors::TryFromBencode;

#[derive(Debug)]
pub struct Torrent {
    pub announce: String,
}

impl TryFrom<Bencode> for Torrent {
    type Error = TryFromBencode;

    fn try_from(value: Bencode) -> Result<Self, Self::Error> {
        let dictionary: BTreeMap<_, _> = value.try_into()?;

        let announce: String = dictionary.get("announce").cloned().unwrap().try_into()?;

        Ok(Torrent { announce })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum Bencode {
    String(Vec<u8>),
    Integer(i64),
    List(Vec<Bencode>),
    Dictionary(BTreeMap<String, Bencode>),
}

impl TryFrom<Bencode> for String {
    type Error = TryFromBencode;

    fn try_from(value: Bencode) -> Result<Self, Self::Error> {
        match value {
            Bencode::String(s) => Ok(std::str::from_utf8(&s)?.into()),
            _ => Err(TryFromBencode::NotString),
        }
    }
}

impl TryFrom<Bencode> for i64 {
    type Error = TryFromBencode;

    fn try_from(value: Bencode) -> Result<Self, Self::Error> {
        match value {
            Bencode::Integer(s) => Ok(s),
            _ => Err(TryFromBencode::NotInteger),
        }
    }
}

impl TryFrom<Bencode> for Vec<Bencode> {
    type Error = TryFromBencode;

    fn try_from(value: Bencode) -> Result<Self, Self::Error> {
        match value {
            Bencode::List(s) => Ok(s),
            _ => Err(TryFromBencode::NotList),
        }
    }
}

impl TryFrom<Bencode> for BTreeMap<String, Bencode> {
    type Error = TryFromBencode;

    fn try_from(value: Bencode) -> Result<Self, Self::Error> {
        match value {
            Bencode::Dictionary(s) => Ok(s),
            _ => Err(TryFromBencode::NotDictionary),
        }
    }
}

impl From<std::str::Utf8Error> for TryFromBencode {
    fn from(value: std::str::Utf8Error) -> Self {
        TryFromBencode::NotUtf8(value)
    }
}
