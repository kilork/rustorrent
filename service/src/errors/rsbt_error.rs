use crate::{
    command::Command,
    errors::TryFromBencode,
    types::{MessageCodecError, UdpTrackerCodecError},
};
use failure::*;
use log::error;

#[derive(Debug, Fail)]
pub enum RsbtError {
    #[fail(display = "io error {}", _0)]
    IO(std::io::Error),
    #[fail(display = "try from bencode {}", _0)]
    TryFromBencode(TryFromBencode),
    #[fail(display = "convert {}", _0)]
    Convert(std::convert::Infallible),
    #[fail(display = "convert integer {}", _0)]
    ConvertInt(std::num::TryFromIntError),
    #[fail(display = "convert from slice {}", _0)]
    ConvertFromSlice(core::array::TryFromSliceError),
    #[fail(display = "HTTP client {}", _0)]
    HTTPClient(hyper::Error),
    #[fail(display = "parser fail")]
    Parser,
    #[fail(display = "tokio send error")]
    TokioMpscSendError,
    #[fail(display = "failure because of: {}", _0)]
    FailureReason(String),
    #[fail(display = "timer failure: {}", _0)]
    TimerFailure(tokio::time::Error),
    #[fail(display = "wrong config")]
    WrongConfig,
    #[fail(display = "send error {}", _0)]
    SendError(futures::channel::mpsc::SendError),
    #[fail(display = "cannot send to torrent process: {}", _0)]
    SendToTorrentToken(tokio::sync::mpsc::error::SendError<Command>),
    #[fail(display = "oneshot recv error {}", _0)]
    TokioMpscOneshotRecvError(tokio::sync::oneshot::error::RecvError),
    #[fail(display = "cannot parse uri {}", _0)]
    InvalidUri(http::uri::InvalidUri),
    #[fail(display = "aborted")]
    Aborted,
    #[fail(display = "peer handshake failure")]
    PeerHandshakeFailure,
    #[fail(display = "message codec {}", _0)]
    MessageCodec(MessageCodecError),
    #[fail(display = "udp tracker codec {}", _0)]
    UdpTrackerCodec(UdpTrackerCodecError),
    #[fail(display = "udp tracker timeout")]
    UdpTrackerTimeout,
    #[fail(display = "udp tracker implementation")]
    UdpTrackerImplementation,
    #[fail(display = "cannot determine announce protocol")]
    AnnounceProtocolFailure,
    #[fail(display = "unknown announce protocol {}", _0)]
    AnnounceProtocolUnknown(String),
    #[fail(display = "join task {}", _0)]
    JoinError(tokio::task::JoinError),
    #[fail(display = "storage {}", _0)]
    Storage(flat_storage::FlatStorageError),
    #[fail(display = "storage version {} is unsupported", _0)]
    StorageVersion(u8),
    #[fail(display = "failure {}", _0)]
    Failure(failure::Context<String>),
    #[fail(display = "toml deserialize {}", _0)]
    TomlDeserialize(toml::de::Error),
    #[fail(display = "toml serialize {}", _0)]
    TomlSerialize(toml::ser::Error),
    #[fail(display = "torrent with id {} not found", _0)]
    TorrentNotFound(usize),
    #[fail(display = "torrent file with id {} not found", _0)]
    TorrentFileNotFound(usize),
    #[fail(display = "torrent file range invalid")]
    TorrentFileRangeInvalid { file_size: usize },
    #[fail(display = "torrent action not supported")]
    TorrentActionNotSupported,
    #[fail(display = "elapsed {}", _0)]
    Elapsed(tokio::time::Elapsed),
    #[fail(display = "bas response from tracker {}", _0)]
    TorrentHttpAnnounceBadResponse(String),
    #[fail(display = "announce failure {}", _0)]
    TorrentHttpAnnounceFailure(hyper::Error),
}

macro_rules! from_rsbt_error {
    ($i:ty, $g:ident) => {
        impl From<$i> for RsbtError {
            fn from(value: $i) -> Self {
                RsbtError::$g(value)
            }
        }
    };
}

from_rsbt_error!(hyper::Error, HTTPClient);
from_rsbt_error!(TryFromBencode, TryFromBencode);
from_rsbt_error!(std::io::Error, IO);
from_rsbt_error!(std::convert::Infallible, Convert);
from_rsbt_error!(std::num::TryFromIntError, ConvertInt);
from_rsbt_error!(core::array::TryFromSliceError, ConvertFromSlice);
from_rsbt_error!(tokio::time::Error, TimerFailure);
from_rsbt_error!(tokio::task::JoinError, JoinError);
from_rsbt_error!(
    tokio::sync::oneshot::error::RecvError,
    TokioMpscOneshotRecvError
);
from_rsbt_error!(futures::channel::mpsc::SendError, SendError);
from_rsbt_error!(http::uri::InvalidUri, InvalidUri);
from_rsbt_error!(MessageCodecError, MessageCodec);
from_rsbt_error!(UdpTrackerCodecError, UdpTrackerCodec);
from_rsbt_error!(flat_storage::FlatStorageError, Storage);
from_rsbt_error!(failure::Context<String>, Failure);
from_rsbt_error!(toml::de::Error, TomlDeserialize);
from_rsbt_error!(toml::ser::Error, TomlSerialize);
from_rsbt_error!(tokio::time::Elapsed, Elapsed);

impl From<futures::future::Aborted> for RsbtError {
    fn from(_: futures::future::Aborted) -> Self {
        RsbtError::Aborted
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for RsbtError {
    fn from(_: tokio::sync::mpsc::error::SendError<T>) -> Self {
        RsbtError::TokioMpscSendError
    }
}

impl<'a> From<nom::Err<&'a [u8]>> for RsbtError {
    fn from(_: nom::Err<&'a [u8]>) -> Self {
        RsbtError::Parser
    }
}

impl<'a> From<nom::Err<(&'a [u8], nom::error::ErrorKind)>> for RsbtError {
    fn from(value: nom::Err<(&'a [u8], nom::error::ErrorKind)>) -> Self {
        error!("{:?}", value);
        RsbtError::Parser
    }
}
