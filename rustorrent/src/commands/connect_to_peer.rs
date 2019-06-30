use super::*;
use crate::types::peer::Handshake;
use crate::KEEP_ALIVE_INTERVAL;
use crate::PEER_ID;

impl Inner {
    pub(crate) fn command_connect_to_peer(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
    ) -> Result<(), RustorrentError> {
        let (tx, rx) = channel(10);

        *torrent_peer.state.lock().unwrap() = TorrentPeerState::Connecting;

        let conntx = tx.clone();
        let addr = torrent_peer.addr;
        let task_keepalive =
            Interval::new(Instant::now() + KEEP_ALIVE_INTERVAL, KEEP_ALIVE_INTERVAL)
                .for_each(move |_| {
                    debug!("Peer {}: sending message KeepAlive", addr);
                    let conntx = conntx.clone();
                    conntx.send(Message::KeepAlive).map(|_| ()).map_err(|err| {
                        error!("Error in KeepAlive send, return shutdown: {}", err);
                        tokio::timer::Error::shutdown()
                    })
                })
                .map_err(move |e| error!("Peer {}: interval errored; err={:?}", addr, e));

        let torrent_process_handshake = torrent_process.clone();
        let torrent_peer_handshake_done = torrent_peer.clone();
        let conntx_state = tx.clone();
        let tcp_stream = TcpStream::connect(&addr)
            .and_then(move |stream| {
                let mut buf = vec![];
                buf.extend_from_slice(&crate::types::HANDSHAKE_PREFIX);
                buf.extend_from_slice(&torrent_process_handshake.hash_id);
                buf.extend_from_slice(&PEER_ID);
                tokio::io::write_all(stream, buf)
            })
            .and_then(move |(stream, buf)| {
                debug!(
                    "Handshake sent to {} (url encoded): {} (len: {})",
                    addr,
                    percent_encode(&buf, SIMPLE_ENCODE_SET).to_string(),
                    buf.len()
                );
                tokio::io::read_exact(stream, vec![0; 68])
            })
            .map_err(move |err| error!("Peer connect to {} failed: {}", addr, err))
            .and_then(move |(stream, buf)| {
                debug!(
                    "Handshake reply from {} (url encoded): {} (len: {})",
                    addr,
                    percent_encode(&buf, SIMPLE_ENCODE_SET).to_string(),
                    buf.len()
                );

                let handshake: Handshake = buf.try_into().unwrap();

                if handshake.info_hash != torrent_process.hash_id {
                    error!("Peer {}: hash is wrong. Disconnect.", addr);
                    return Err(());
                }

                let (writer, reader) = stream.framed(MessageCodec::default()).split();

                let writer = writer.sink_map_err(|err| error!("Error in sink channel: {}", err));

                let sink = rx.forward(writer).inspect(move |(_a, _sink)| {
                    debug!("Peer {}: updated", addr);
                });
                tokio::spawn(sink.map(|_| ()));

                *torrent_peer_handshake_done.state.lock().unwrap() = TorrentPeerState::Connected {
                    chocked: true,
                    interested: false,
                    downloading: false,
                    sender: conntx_state.clone(),
                    pieces: vec![],
                };

                let conn = reader
                    .for_each(move |frame| {
                        debug!("Peer {}: received message {}", addr, frame);
                        match frame {
                            Message::KeepAlive => {
                                let conntx = tx.clone();
                                tokio::spawn(conntx.send(Message::KeepAlive).map(|_| ()).map_err(
                                    move |e| {
                                        error!(
                                            "Peer {}: Cannot send KeepAlive message: {:?}",
                                            addr, e
                                        )
                                    },
                                ));
                            }
                            message => {
                                let peer_message = RustorrentCommand::PeerMessage(
                                    torrent_process.clone(),
                                    torrent_peer_handshake_done.clone(),
                                    message,
                                );
                                self.clone().send_command(peer_message).unwrap();
                            }
                        }
                        Ok(())
                    })
                    .map_err(move |err| error!("Peer {}: message codec error: {}", addr, err));

                tokio::spawn(conn);

                Ok(())
            });

        tokio::spawn(tcp_stream.join(task_keepalive).map(|_| ()).then(move |_| {
            info!("Peer {} is done", addr);

            *torrent_peer.state.lock().unwrap() = TorrentPeerState::Idle;

            Ok(())
        }));

        Ok(())
    }
}
