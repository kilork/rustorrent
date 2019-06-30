use super::*;

impl Inner {
    pub(crate) fn command_piece_downloaded(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        piece: usize,
    ) -> Result<(), RustorrentError> {
        info!("Piece {} downloaded, checking SHA1", piece);
        Ok(())
    }
}
