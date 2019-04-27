pub mod app;
mod errors;
mod parser;
pub mod types;

use std::convert::TryInto;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use errors::RustorrentError;
use types::torrent::Torrent;
