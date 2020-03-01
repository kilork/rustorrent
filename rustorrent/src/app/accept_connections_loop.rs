use super::*;

pub(crate) async fn accept_connections_loop(
    settings: Arc<Settings>,
    addr: SocketAddr,
    sender: Sender<RustorrentCommand>,
) -> Result<(), RustorrentError> {
    debug!("listening on: {}", &addr);
    let mut listener = TcpListener::bind(addr).await?;

    loop {
        let (socket, _) = listener.accept().await?;
        let _ = spawn_and_log_error(
            peer_connection(settings.clone(), socket, sender.clone()),
            move || format!("peer connection {} failed", addr),
        );
    }
}
