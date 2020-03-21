use futures::{
    future::{join, AbortHandle, Abortable},
    prelude::*,
    stream::SplitSink,
    try_join,
};
use http_body::Body;
use hyper::Client;
use log::{debug, error};
use percent_encoding::{percent_encode, percent_encode_byte, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    convert::TryInto,
    fmt::{Display, Formatter},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    fs,
    net::{TcpListener, TcpStream, UdpSocket},
    prelude::*,
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
    time::delay_for,
};
use tokio_util::{codec::Framed, udp::UdpFramed};
use uuid::Uuid;

pub mod announce;
pub mod app;
mod errors;
mod messages;
mod parser;
mod storage;
pub mod types;

pub use errors::RsbtError;
pub use storage::{TorrentPiece, TorrentStorage};

pub(crate) const SHA1_SIZE: usize = 20;

pub(crate) const BLOCK_SIZE: usize = 1 << 14;

pub(crate) const PEER_ID: [u8; 20] = *b"-rs0001-zzzzxxxxyyyy";

//FIXME: pub(crate) const PEER_MAX_CONNECTIONS: usize = 50;

pub const DEFAULT_CHANNEL_BUFFER: usize = 256;

//FIXME: pub(crate) const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(110);

pub(crate) fn count_parts(total: usize, part_size: usize) -> usize {
    total / part_size + if total % part_size != 0 { 1 } else { 0 }
}

pub fn default_app_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".rsbt")
}
