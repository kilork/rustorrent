use super::*;
use bytes::Bytes;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread,
    time::Duration,
};

#[derive(Debug)]
pub struct RsbtFileDownloadStream {
    pub name: String,
    pub size: usize,
}

impl Stream for RsbtFileDownloadStream {
    type Item = Result<Bytes, RsbtError>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        unimplemented!()
    }
}

pub(crate) async fn torrent_file_download(
    request: &RsbtCommandTorrentFileDownload,
    torrents: &[TorrentDownload],
) -> Result<RsbtFileDownloadStream, RsbtError> {
    let torrent = find_torrent(torrents, request.id)?;
    torrent.download_file(request.file_id).await
}

impl TorrentDownload {
    async fn download_file(&self, file_id: usize) -> Result<RsbtFileDownloadStream, RsbtError> {
        debug!("files for {}", self.id);
        self.request(file_id, DownloadTorrentEvent::FileDownload)
            .await
    }
}
