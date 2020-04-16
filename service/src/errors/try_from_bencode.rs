use failure::*;

#[derive(Debug, Fail)]
pub enum TryFromBencode {
    #[fail(display = "not a string bencode")]
    NotString,
    #[fail(display = "not an integer bencode")]
    NotInteger,
    #[fail(display = "not a list bencode")]
    NotList,
    #[fail(display = "not a dictionary bencode")]
    NotDictionary,
    #[fail(display = "not valid utf-8 {}", _0)]
    NotUtf8(std::str::Utf8Error),
    #[fail(display = "not valid ip {}", _0)]
    NotValidIp(std::net::AddrParseError),
}
