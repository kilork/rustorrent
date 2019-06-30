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

        if let Some(command) = match message {
            Message::Bitfield(bitfield_pieces) => {
                message_bitfield(torrent_process, torrent_peer, bitfield_pieces)?
            }
            Message::Unchoke => message_unchoke(torrent_process, torrent_peer)?,
            _ => {
                warn!("Unsupported message {:?}", message);
                None
            }
        } {
            self.clone().send_command(command)?;
        }

        Ok(())
    }
}
