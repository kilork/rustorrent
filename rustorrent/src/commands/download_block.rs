use super::*;

impl Inner {
    pub(crate) fn command_download_block(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
        block: Block,
    ) -> Result<(), RustorrentError> {
        info!("Received command to download block: {:?}", &block);

        Ok(())
    }
}
