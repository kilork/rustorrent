use super::*;
use crate::messages::*;

impl Inner {
    pub(crate) fn command_peer_message(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
        message: Message,
    ) -> Result<(), RustorrentError> {
        info!("Handle message: {:?}", message);

        match message {
            Message::Bitfield(bitfield_pieces) => {
                message_bitfield(torrent_process, torrent_peer, bitfield_pieces)?;
            }
            _ => warn!("Unsupported message {:?}", message),
        }

        Ok(())
    }
}
