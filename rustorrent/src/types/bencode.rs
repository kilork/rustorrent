use super::*;
use crate::errors::RustorrentError;
use crate::parser::parse_bencode;
use std::convert::TryInto;
use std::net::Ipv4Addr;

#[derive(Debug, PartialEq, Clone)]
pub struct BencodeBlob {
    pub source: Vec<u8>,
    pub value: BencodeValue,
}

impl TryFrom<Vec<u8>> for BencodeBlob {
    type Error = RustorrentError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        parse_bencode(&value)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum BencodeValue {
    String(Vec<u8>),
    Integer(i64),
    List(Vec<BencodeBlob>),
    Dictionary(Vec<(String, BencodeBlob)>),
}

macro_rules! blanket_blob_value {
    ($type:ty) => {
        impl TryFrom<BencodeBlob> for $type {
            type Error = TryFromBencode;

            fn try_from(value: BencodeBlob) -> Result<Self, Self::Error> {
                BencodeValue::from(value).try_into()
            }
        }
    };
}

impl TryFrom<BencodeValue> for String {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::String(s) => Ok(std::str::from_utf8(&s)?.into()),
            _ => Err(TryFromBencode::NotString),
        }
    }
}
blanket_blob_value!(String);

impl TryFrom<BencodeValue> for i64 {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::Integer(s) => Ok(s),
            _ => Err(TryFromBencode::NotInteger),
        }
    }
}
blanket_blob_value!(i64);

impl TryFrom<BencodeValue> for u16 {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue) -> Result<Self, Self::Error> {
        value.try_into().map(|x: i64| x as u16)
    }
}
blanket_blob_value!(u16);

impl TryFrom<BencodeValue> for Vec<BencodeBlob> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::List(s) => Ok(s),
            _ => Err(TryFromBencode::NotList),
        }
    }
}
blanket_blob_value!(Vec<BencodeBlob>);

impl TryFrom<BencodeValue> for Vec<(String, BencodeBlob)> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::Dictionary(s) => Ok(s),
            _ => Err(TryFromBencode::NotDictionary),
        }
    }
}
blanket_blob_value!(Vec<(String, BencodeBlob)>);

impl TryFrom<BencodeValue> for Vec<String> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::List(s) => Ok(s.into_iter().map(|i| i.try_into().unwrap()).collect()),
            _ => Err(TryFromBencode::NotDictionary),
        }
    }
}
blanket_blob_value!(Vec<String>);

impl TryFrom<BencodeValue> for Vec<Vec<String>> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue) -> Result<Self, Self::Error> {
        value.try_into()
    }
}
blanket_blob_value!(Vec<Vec<String>>);

impl TryFrom<BencodeValue> for Ipv4Addr {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue) -> Result<Self, Self::Error> {
        value
            .try_into()
            .and_then(|s: String| s.parse().map_err(TryFromBencode::from))
    }
}
blanket_blob_value!(Ipv4Addr);

impl From<BencodeBlob> for BencodeValue {
    fn from(blob: BencodeBlob) -> Self {
        blob.value
    }
}

impl From<std::str::Utf8Error> for TryFromBencode {
    fn from(value: std::str::Utf8Error) -> Self {
        TryFromBencode::NotUtf8(value)
    }
}

impl From<std::net::AddrParseError> for TryFromBencode {
    fn from(value: std::net::AddrParseError) -> Self {
        TryFromBencode::NotValidIp(value)
    }
}

macro_rules! try_from_bencode {
    ($type:ty,
        $(normal: ($($normal_key:expr => $normal_field:ident),*)$(,)*)*
        $(optional: ($($optional_key:expr => $optional_field:ident),*)$(,)*)*
        $(bencode: ($($bencode_key:expr => $bencode_field:ident),*)$(,)*)*
        $(raw: ($($raw:ident),*))*) => {
        impl TryFrom<BencodeBlob> for $type {

            type Error = TryFromBencode;

            fn try_from(value: BencodeBlob) -> Result<Self, Self::Error> {
                let _source = value.source.clone();
                let dictionary: Vec<_> = value.try_into()?;

                $($(let mut $normal_field = None;)*)*
                $($(let mut $optional_field = None;)*)*
                $($(let mut $bencode_field = None;)*)*

                for (key, value) in dictionary {
                    match key.as_str() {
                        $($($normal_key => $normal_field = Some(value.try_into()?),)*)*
                        $($($optional_key => $optional_field = Some(value.try_into()?),)*)*
                        $($($bencode_key => $bencode_field = Some(value),)*)*
                        _ => (),
                    }
                }

                Ok(Self {
                    $($($raw: _source.to_vec(),)*)*
                    $($($normal_field: $normal_field.unwrap(),)*)*
                    $($($optional_field,)*)*
                    $($($bencode_field: $bencode_field.unwrap(),)*)*
                })
            }
        }

        impl TryFrom<Vec<u8>> for $type {
            type Error = RustorrentError;

            fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
                let bencode: BencodeBlob = value.try_into()?;
                bencode.try_into().map_err(RustorrentError::from)
            }
        }
    }
}
