use serde::Serialize;
use std::borrow::Cow;

#[derive(Serialize)]
pub struct BackendTorrentDownload<'a> {
    pub id: usize,
    pub name: Cow<'a, str>,
    pub received: usize,
    pub uploaded: usize,
    pub length: usize,
    pub active: bool,
}

#[derive(Serialize)]
pub struct BackendTorrentDownloadDetail {}
