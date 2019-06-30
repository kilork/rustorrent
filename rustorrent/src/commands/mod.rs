use crate::app::*;
use crate::errors::RustorrentError;
use crate::types::message::{Message, MessageCodec, MessageCodecError};
use crate::types::torrent::parse_torrent;
use crate::types::torrent::TrackerAnnounce;
use crate::PEER_ID;
use exitfailure::ExitFailure;
use failure::{Context, ResultExt};
use futures::future::join_all;
use futures::lazy;
use futures::prelude::*;
use futures::sync::mpsc::{channel, Receiver, Sender};
use log::{debug, error, info, warn};
use percent_encoding::{percent_encode, percent_encode_byte, SIMPLE_ENCODE_SET};
use reqwest::r#async::{Client, Decoder as ReqwestDecoder};
use std::collections::HashMap;
use std::convert::TryInto;
use std::mem;
use std::mem::drop;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tokio::codec::Decoder;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::timer::{Delay, Interval};

mod add_torrent;
mod connect_to_peer;
mod download_block;
mod peer_message;
mod piece_downloaded;
mod process_announce;
mod start_announce_process;

fn url_encode(data: &[u8]) -> String {
    data.iter()
        .map(|&x| percent_encode_byte(x))
        .collect::<String>()
}
