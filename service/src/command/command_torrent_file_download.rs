use std::ops::Range;

#[derive(Debug)]
pub struct CommandTorrentFileDownload {
    pub id: usize,
    pub file_id: usize,
    pub range: Option<Range<usize>>,
}
