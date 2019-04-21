use std::convert::{TryFrom, TryInto};
use std::ops::Deref;

use crate::errors::TryFromBencode;

#[derive(Debug)]
pub struct Torrent<'a> {
    pub raw: &'a [u8],
    pub announce: &'a str,
    pub announce_list: Option<Vec<Vec<&'a str>>>,
    pub creation_date: Option<i64>,
}

impl<'a> TryFrom<BencodeBlob<'a>> for Torrent<'a> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeBlob<'a>) -> Result<Self, Self::Error> {
        let dictionary: Vec<_> = value.value.try_into()?;

        let mut announce = None;
        let mut announce_list = None;
        let mut creation_date = None;

        for (key, value) in dictionary {
            match key {
                "announce" => announce = Some(value.value.try_into()?),
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
                _ => (),
            }
        }

        Ok(Torrent {
            raw: value.source,
            announce: announce.unwrap(),
            announce_list,
            creation_date,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct BencodeBlob<'a> {
    pub source: &'a [u8],
    pub value: BencodeValue<'a>,
}

impl<'a> Deref for BencodeBlob<'a> {
    type Target = BencodeValue<'a>;

    fn deref(&self) -> &BencodeValue<'a> {
        &self.value
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum BencodeValue<'a> {
    String(&'a [u8]),
    Integer(i64),
    List(Vec<BencodeBlob<'a>>),
    Dictionary(Vec<(&'a str, BencodeBlob<'a>)>),
}

impl<'a> TryFrom<BencodeValue<'a>> for &'a str {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::String(s) => Ok(std::str::from_utf8(&s)?),
            _ => Err(TryFromBencode::NotString),
        }
    }
}

impl<'a> TryFrom<BencodeValue<'a>> for i64 {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::Integer(s) => Ok(s),
            _ => Err(TryFromBencode::NotInteger),
        }
    }
}

impl<'a> TryFrom<BencodeValue<'a>> for Vec<BencodeBlob<'a>> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::List(s) => Ok(s),
            _ => Err(TryFromBencode::NotList),
        }
    }
}

impl<'a> TryFrom<BencodeValue<'a>> for Vec<(&'a str, BencodeBlob<'a>)> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::Dictionary(s) => Ok(s),
            _ => Err(TryFromBencode::NotDictionary),
        }
    }
}

impl From<std::str::Utf8Error> for TryFromBencode {
    fn from(value: std::str::Utf8Error) -> Self {
        TryFromBencode::NotUtf8(value)
    }
}
