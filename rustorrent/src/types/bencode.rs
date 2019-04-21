use super::*;

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
