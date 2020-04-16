mod torrent_piece;
mod torrent_storage;
mod torrent_storage_message;
mod torrent_storage_state;

const TORRENT_STORAGE_FORMAT_VERSION: u8 = 0;

pub use torrent_piece::TorrentPiece;
pub use torrent_storage::TorrentStorage;
use torrent_storage_message::TorrentStorageMessage;
pub use torrent_storage_state::TorrentStorageState;
