use super::*;
use std::path::PathBuf;

mod add_torrent;

use add_torrent::add_torrent;

pub(crate) async fn download_events_loop(
    properties: Arc<Properties>,
    mut events: Receiver<RsbtCommand>,
) {
    let mut torrents = vec![];
    let mut id = 0;

    while let Some(event) = events.next().await {
        match event {
            RsbtCommand::AddTorrent(request_response, filename) => {
                let torrent = add_torrent(
                    properties.clone(),
                    &request_response,
                    filename,
                    &mut id,
                    &mut torrents,
                )
                .await;
                if let Err(err) = request_response.response(torrent) {
                    error!("cannot send response for add torrent: {}", err);
                }
            }
            RsbtCommand::TorrentHandshake {
                handshake_request,
                handshake_sender,
            } => {
                debug!("searching for matching torrent handshake");

                let hash_id = handshake_request.info_hash;

                if handshake_sender
                    .send(
                        torrents
                            .iter()
                            .map(|x| &x.process)
                            .find(|x| x.hash_id == hash_id)
                            .cloned(),
                    )
                    .is_err()
                {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
            RsbtCommand::TorrentList { sender } => {
                debug!("collecting torrent list");
                if sender.send(torrents.to_vec()).is_err() {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
        }
    }

    debug!("download_events_loop done");
}

