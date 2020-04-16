#[derive(Clone, Copy, Debug)]
pub struct TorrentDownloadState {
    pub downloaded: u64,
    pub uploaded: u64,
}
