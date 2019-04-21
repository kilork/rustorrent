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
    #[fail(display = "convert {}", _0)]
    Convert(std::convert::Infallible),
    #[fail(display = "HTTP client {}", _0)]
    HTTPClient(reqwest::Error),
}

macro_rules! from_rustorrent_error {
    ($i:ty, $g:ident) => {
        impl From<$i> for RustorrentError {
            fn from(value: $i) -> Self {
                RustorrentError::$g(value)
            }
        }
    };
}

from_rustorrent_error!(reqwest::Error, HTTPClient);
from_rustorrent_error!(TryFromBencode, TryFromBencode);
from_rustorrent_error!(std::io::Error, IO);
from_rustorrent_error!(std::convert::Infallible, Convert);
