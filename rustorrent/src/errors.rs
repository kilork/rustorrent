use failure::*;
use log::error;

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

#[derive(Debug, Fail)]
pub enum RustorrentError {
    #[fail(display = "io error {}", _0)]
    IO(std::io::Error),
    #[fail(display = "try from bencode {}", _0)]
    TryFromBencode(TryFromBencode),
    #[fail(display = "convert {}", _0)]
    Convert(std::convert::Infallible),
    #[fail(display = "convert from slice {}", _0)]
    ConvertFromSlice(core::array::TryFromSliceError),
    #[fail(display = "HTTP client {}", _0)]
    HTTPClient(reqwest::Error),
    #[fail(display = "parser fail")]
    Parser,
    #[fail(display = "tokio unbounded receiver {}", _0)]
    TokioMpscUnboundedRecvError(tokio::sync::mpsc::error::UnboundedRecvError)
}

macro_rules! from_rustorrent_error {
    ($i:ty, $g:ident) => {
        impl From<$i> for RustorrentError {
            fn from(value: $i) -> Self {
                error!("{}", value);
                RustorrentError::$g(value)
            }
        }
    };
}

from_rustorrent_error!(reqwest::Error, HTTPClient);
from_rustorrent_error!(TryFromBencode, TryFromBencode);
from_rustorrent_error!(std::io::Error, IO);
from_rustorrent_error!(std::convert::Infallible, Convert);
from_rustorrent_error!(core::array::TryFromSliceError, ConvertFromSlice);
from_rustorrent_error!(tokio::sync::mpsc::error::UnboundedRecvError, TokioMpscUnboundedRecvError);

impl<'a> From<nom::Err<&'a [u8]>> for RustorrentError {
    fn from(_value: nom::Err<&'a [u8]>) -> Self {
        error!("{}", _value);
        RustorrentError::Parser
    }
}
