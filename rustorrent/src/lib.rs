pub mod app;
mod errors;
mod parser;
pub mod types;

pub use errors::RustorrentError;

pub(crate) const SHA1_SIZE: usize = 20;

pub(crate) const BLOCK_SIZE: usize = 1 << 14;
