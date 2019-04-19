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
}

#[derive(Debug, Fail)]
pub enum RustorrentError {
    #[fail(display = "io error {}", _0)]
    IO(std::io::Error),
    #[fail(display = "try from bencode {}", _0)]
    TryFromBencode(TryFromBencode),
}

impl From<std::io::Error> for RustorrentError {
    fn from(value: std::io::Error) -> Self {
        RustorrentError::IO(value)
    }
}

impl From<TryFromBencode> for RustorrentError {
    fn from(value: TryFromBencode) -> Self {
        RustorrentError::TryFromBencode(value)
    }
}
