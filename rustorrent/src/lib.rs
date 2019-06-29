pub mod app;
mod commands;
mod errors;
mod messages;
mod parser;
pub mod types;

use std::time::Duration;

pub use errors::RustorrentError;

pub(crate) const SHA1_SIZE: usize = 20;

pub(crate) const BLOCK_SIZE: usize = 1 << 14;

pub(crate) const PEER_ID: [u8; 20] = *b"-rs0001-zzzzxxxxyyyy";

pub(crate) const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(110);
