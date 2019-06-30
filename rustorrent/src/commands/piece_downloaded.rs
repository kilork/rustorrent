use super::*;

impl Inner {
    pub(crate) fn command_piece_downloaded(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        piece: usize,
    ) -> Result<(), RustorrentError> {
        info!("Piece {} downloaded, checking SHA1", piece);
        unimplemented!();
        // TODO: 1. check overall progress and update
        // TODO: 2. select next block to download
        // TODO: 3. send Have message
    }
}
