use exitfailure::ExitFailure;
use failure::{Context, ResultExt};
use futures::{
    // channel::mpsc::{self, UnboundedReceiver, UnboundedSender},
    future::{join_all, lazy, try_join, AbortHandle, Abortable, Aborted},
    join,
    prelude::*,
    stream::SplitSink,
    task::{FutureObj, Spawn, SpawnError, SpawnExt},
    try_join,
};
use http_body::Body;
use hyper::{Client, Uri};
use log::{debug, error, info, warn};
use percent_encoding::{percent_encode, percent_encode_byte, NON_ALPHANUMERIC};
use sha1::{Digest, Sha1};
use std::{
    collections::HashMap,
    convert::TryInto,
    fmt::{Display, Formatter},
    mem::{self, drop},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Deref,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};
use tokio::{
    io,
    net::{TcpListener, TcpStream},
    prelude::*,
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot,
    },
    task::JoinHandle,
    time::{delay_for, Interval},
};
use tokio_util::codec::{Decoder, Encoder, Framed};
use uuid::Uuid;

pub mod app;
mod commands;
mod errors;
mod messages;
mod parser;
mod storage;
pub mod types;
pub mod announce;

pub use errors::RustorrentError;
pub use storage::{TorrentPiece, TorrentStorage};

pub(crate) const SHA1_SIZE: usize = 20;

pub(crate) const BLOCK_SIZE: usize = 1 << 14;

pub(crate) const PEER_ID: [u8; 20] = *b"-rs0001-zzzzxxxxyyyy";

pub(crate) const PEER_MAX_CONNECTIONS: usize = 50;

pub(crate) const DEFAULT_CHANNEL_BUFFER: usize = 10;

pub(crate) const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(110);

pub(crate) fn count_parts(total: usize, part_size: usize) -> usize {
    total / part_size + if total % part_size != 0 { 1 } else { 0 }
}
