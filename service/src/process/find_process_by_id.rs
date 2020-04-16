use crate::{process::TorrentProcess, RsbtError};

pub(crate) fn find_process_by_id(
    torrents: &[TorrentProcess],
    id: usize,
) -> Result<&TorrentProcess, RsbtError> {
    torrents
        .iter()
        .find(|x| x.id == id)
        .ok_or_else(|| RsbtError::TorrentNotFound(id))
}
