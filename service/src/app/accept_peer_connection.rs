use crate::{command::Command, event::TorrentEvent, types::Handshake, RsbtError};
use log::{debug, error};
use std::convert::TryInto;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{mpsc::Sender, oneshot},
};

pub(crate) async fn accept_peer_connection(
    mut socket: TcpStream,
    mut sender: Sender<Command>,
) -> Result<(), RsbtError> {
    let mut handshake_request = vec![0u8; 68];

    socket.read_exact(&mut handshake_request).await?;

    let handshake_request: Handshake = handshake_request.try_into()?;

    let (handshake_sender, handshake_receiver) = oneshot::channel();

    sender
        .send(Command::TorrentHandshake {
            handshake_request,
            handshake_sender,
        })
        .await?;

    let torrent_process = match handshake_receiver.await {
        Ok(Some(torrent_process)) => torrent_process,
        Ok(None) => {
            debug!("torrent not found, closing connection");
            return Ok(());
        }
        Err(err) => {
            error!("cannot send message to torrent download queue: {}", err);
            return Err(RsbtError::PeerHandshakeFailure);
        }
    };

    socket.write_all(&torrent_process.handshake).await?;

    debug!("handshake done, connected with peer");

    torrent_process
        .broker_sender
        .clone()
        .send(TorrentEvent::PeerForwarded(socket))
        .await?;

    Ok(())
}
