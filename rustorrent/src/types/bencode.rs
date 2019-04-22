use super::*;
use crate::errors::RustorrentError;
use crate::parser::parse_bencode;
use std::convert::TryInto;

#[derive(Debug, PartialEq, Clone)]
pub struct BencodeBlob<'a> {
    pub source: &'a [u8],
    pub value: BencodeValue<'a>,
}

impl<'a> TryFrom<&'a [u8]> for BencodeBlob<'a> {
    type Error = RustorrentError;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        parse_bencode(value)
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum BencodeValue<'a> {
    String(&'a [u8]),
    Integer(i64),
    List(Vec<BencodeBlob<'a>>),
    Dictionary(Vec<(&'a str, BencodeBlob<'a>)>),
}

macro_rules! blanket_blob_value {
    ($type:ty) => {
        impl<'a> TryFrom<BencodeBlob<'a>> for $type {
            type Error = TryFromBencode;

            fn try_from(value: BencodeBlob<'a>) -> Result<Self, Self::Error> {
                BencodeValue::from(value).try_into()
            }
        }
    };
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
blanket_blob_value!(&'a str);

impl<'a> TryFrom<BencodeValue<'a>> for i64 {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::Integer(s) => Ok(s),
            _ => Err(TryFromBencode::NotInteger),
        }
    }
}
blanket_blob_value!(i64);

impl<'a> TryFrom<BencodeValue<'a>> for Vec<BencodeBlob<'a>> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::List(s) => Ok(s),
            _ => Err(TryFromBencode::NotList),
        }
    }
}
blanket_blob_value!(Vec<BencodeBlob<'a>>);

impl<'a> TryFrom<BencodeValue<'a>> for Vec<(&'a str, BencodeBlob<'a>)> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::Dictionary(s) => Ok(s),
            _ => Err(TryFromBencode::NotDictionary),
        }
    }
}
blanket_blob_value!(Vec<(&'a str, BencodeBlob<'a>)>);

impl<'a> TryFrom<BencodeValue<'a>> for Vec<&'a str> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        match value {
            BencodeValue::List(s) => Ok(s.into_iter().map(|i| i.try_into().unwrap()).collect()),
            _ => Err(TryFromBencode::NotDictionary),
        }
    }
}
blanket_blob_value!(Vec<&'a str>);

impl<'a> TryFrom<BencodeValue<'a>> for Vec<Vec<&'a str>> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeValue<'a>) -> Result<Self, Self::Error> {
        value.try_into()
    }
}
blanket_blob_value!(Vec<Vec<&'a str>>);

impl<'a> From<BencodeBlob<'a>> for BencodeValue<'a> {
    fn from(blob: BencodeBlob<'a>) -> Self {
        blob.value
    }
}

impl From<std::str::Utf8Error> for TryFromBencode {
    fn from(value: std::str::Utf8Error) -> Self {
        TryFromBencode::NotUtf8(value)
    }
}

macro_rules! try_from_bencode {
    ($type:ty,
        $(normal: ($($normal_key:expr => $normal_field:ident),*),)*
        $(optional: ($($optional_key:expr => $optional_field:ident),*),)*
        $(bencode: ($($bencode_key:expr => $bencode_field:ident),*),)*
        $(raw: ($($raw:ident),*))*) => {
        impl<'a> TryFrom<BencodeBlob<'a>> for $type {
            type Error = TryFromBencode;
            fn try_from(value: BencodeBlob<'a>) -> Result<Self, Self::Error> {
                let _source = value.source.clone();
                let dictionary: Vec<_> = value.try_into()?;

                $($(let mut $normal_field = None;)*)*
                $($(let mut $optional_field = None;)*)*
                $($(let mut $bencode_field = None;)*)*

                for (key, value) in dictionary {
                    match key {
                        $($($normal_key => $normal_field = Some(value.try_into()?),)*)*
                        $($($optional_key => $optional_field = Some(value.try_into()?),)*)*
                        $($($bencode_key => $bencode_field = Some(value),)*)*
                        _ => (),
                    }
                }

                Ok(Self {
                    $($($raw: _source,)*)*
                    $($($normal_field: $normal_field.unwrap(),)*)*
                    $($($optional_field,)*)*
                    $($($bencode_field: $bencode_field.unwrap(),)*)*
                })
            }
        }
    };
}
