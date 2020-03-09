use super::*;

pub(crate) async fn accept_connections_loop(
    addr: SocketAddr,
    sender: Sender<RsbtCommand>,
) -> Result<(), RsbtError> {
    debug!("listening on: {}", &addr);
    let mut listener = TcpListener::bind(addr).await?;

    loop {
        let (socket, _) = listener.accept().await?;
        let _ = spawn_and_log_error(peer_connection(socket, sender.clone()), move || {
            format!("peer connection {} failed", addr)
        });
    }
}
