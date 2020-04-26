use std::path::PathBuf;

mod announce;
mod app;
mod command;
mod errors;
mod event;
mod event_loop;
mod file_download;
mod parser;
mod peer;
mod piece;
mod process;
mod request_response;
mod spawn_and_log_error;
mod storage;
mod types;

pub use app::App as RsbtApp;
pub use command::Command as RsbtCommand;
pub use command::CommandAddTorrent as RsbtCommandAddTorrent;
pub use command::CommandDeleteTorrent as RsbtCommandDeleteTorrent;
pub use command::CommandTorrentAction as RsbtCommandTorrentAction;
pub use command::CommandTorrentAnnounce as RsbtCommandTorrentAnnounce;
pub use command::CommandTorrentDetail as RsbtCommandTorrentDetail;
pub use command::CommandTorrentFileDownload as RsbtCommandTorrentFileDownload;
pub use command::CommandTorrentFiles as RsbtCommandTorrentFiles;
pub use command::CommandTorrentPeers as RsbtCommandTorrentPeers;
pub use command::CommandTorrentPieces as RsbtCommandTorrentPieces;
pub use errors::RsbtError;
pub use process::TorrentProcess as RsbtTorrentProcess;
pub use process::TorrentProcessStatus as RsbtTorrentProcessStatus;
pub use request_response::RequestResponse as RsbtRequestResponse;
pub(crate) use spawn_and_log_error::spawn_and_log_error;
pub use types::public::TorrentAction as RsbtTorrentAction;
pub use types::public::TorrentDownloadView as RsbtTorrentDownloadView;
pub use types::public::TorrentStatisticsEvent as RsbtTorrentStatisticsEvent;
pub use types::Config as RsbtConfig;
pub use types::Properties as RsbtProperties;
pub use types::Settings as RsbtSettings;
pub use types::Torrent as RsbtTorrent;

pub(crate) const SHA1_SIZE: usize = 20;

pub(crate) const BLOCK_SIZE: usize = 1 << 14;

pub(crate) const PEER_ID: [u8; 20] = *b"-rs0001-zzzzxxxxyyyy";

//FIXME: pub(crate) const PEER_MAX_CONNECTIONS: usize = 50;
pub const TORRENTS_TOML: &str = "torrents.toml";

pub const DEFAULT_CHANNEL_BUFFER: usize = 256;

//FIXME: pub(crate) const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(110);

pub(crate) fn count_parts(total: usize, part_size: usize) -> usize {
    total / part_size + if total % part_size != 0 { 1 } else { 0 }
}

pub fn default_app_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".rsbt")
}
