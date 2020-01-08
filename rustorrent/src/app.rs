use super::*;
use crate::{errors::RustorrentError, types::torrent::parse_torrent, PEER_ID};

use crate::{
    types::{
        info::TorrentInfo,
        message::{Message, MessageCodec, MessageCodecError},
        peer::{Handshake, Peer},
        torrent::{Torrent, TrackerAnnounce},
        Settings,
    },
    {commands::url_encode, count_parts, SHA1_SIZE},
};

pub struct RustorrentApp {
    pub settings: Arc<Settings>,
}

#[derive(Debug)]
pub struct TorrentProcess {
    // pub(crate) path: PathBuf,
    pub(crate) torrent: Torrent,
    pub(crate) info: TorrentInfo,
    pub(crate) hash_id: [u8; SHA1_SIZE],
    pub(crate) handshake: Vec<u8>,
    broker_sender: Sender<DownloadTorrentEvent>,
    // pub(crate) torrent_state: Arc<Mutex<TorrentProcessState>>,
    // pub(crate) announce_state: Arc<Mutex<AnnounceState>>,
    // pub(crate) blocks_downloading: Arc<Mutex<HashMap<Block, Arc<TorrentPeer>>>>,
    // pub(crate) stats: Arc<Mutex<TorrentProcessStats>>,
    // pub(crate) torrent_storage: RwLock<TorrentStorage>,
}

#[derive(Debug)]
struct TorrentStorage {
    pieces: Vec<TorrentPiece>,
    downloaded: Vec<u8>,
}

#[derive(Debug)]
enum TorrentPiece {
    Data(Vec<u8>),
}

#[derive(Debug, Default)]
pub(crate) struct TorrentPeerPiece {
    pub(crate) downloaded: bool,
    pub(crate) data: Vec<u8>,
    pub(crate) blocks: Vec<u8>,
    pub(crate) blocks_to_download: usize,
}

impl TorrentPeerPiece {
    pub(crate) fn init(&mut self, piece_length: usize, blocks_count: usize) {
        self.data = vec![0; piece_length];
        self.blocks = vec![0; count_parts(blocks_count, 8)];
        self.blocks_to_download = blocks_count;
    }

    pub(crate) fn init_from_info(&mut self, info: &TorrentInfo, index: usize) {
        let (piece_length, blocks_count) = info.sizes(index);
        self.init(piece_length, blocks_count);
    }
}

#[derive(Debug)]
enum TorrentPeerState {
    Idle,
    Connecting(JoinHandle<()>),
    Connected {
        chocked: bool,
        interested: bool,
        downloading_piece: Option<usize>,
        sender: Sender<PeerMessage>,
        pieces: Vec<u8>,
    },
}

impl Default for TorrentPeerState {
    fn default() -> Self {
        TorrentPeerState::Idle
    }
}

#[derive(Debug)]
pub(crate) struct TorrentProcessStats {
    pub(crate) downloaded: usize,
    pub(crate) uploaded: usize,
    pub(crate) left: usize,
}

#[derive(Debug, PartialEq)]
pub enum TorrentProcessState {
    Init,
    Download,
    DownloadUpload,
    Upload,
    Finished,
}

#[derive(Debug, PartialEq, Hash, Eq)]
pub(crate) struct Block {
    pub piece: u32,
    pub begin: u32,
    pub length: u32,
}

pub(crate) enum RustorrentEvent {
    AddTorrent(PathBuf),
    TorrentHandshake {
        handshake_request: Handshake,
        handshake_sender: oneshot::Sender<Option<Arc<TorrentProcess>>>,
    },
}

struct PeerState {
    peer: Peer,
    state: TorrentPeerState,
    announce_count: usize,
}

#[derive(Debug)]
enum PeerMessage {
    Disconnect,
    Message(Message),
    Download(usize),
}

impl RustorrentApp {
    pub fn new(settings: Settings) -> Self {
        let settings = Arc::new(settings);
        Self { settings }
    }
    /*
        pub fn add_torrent_from_file(
            &self,
            filename: impl AsRef<Path>,
        ) -> Result<Arc<TorrentProcess>, RustorrentError> {
            info!("Adding torrent from file: {:?}", filename.as_ref());
            let torrent = parse_torrent(&filename)?;
            let hash_id = torrent.info_sha1_hash();

            if let Some(process) = match self.processes.read() {
                Ok(processes) => processes,
                Err(poisoned) => poisoned.into_inner(),
            }
            .iter()
            .filter(|x| x.hash_id == hash_id)
            .cloned()
            .next()
            {
                warn!(
                    "Torrent already in the list: {}",
                    crate::commands::url_encode(&hash_id)
                );
                return Ok(process);
            }

            let info = torrent.info()?;
            let left = info.len();
            let pieces_count = info.pieces.len();
            let pieces = (0..pieces_count)
                .map(|_| Arc::new(Mutex::new(Default::default())))
                .collect();

            let process = Arc::new(TorrentProcess {
                path: filename.as_ref().into(),
                torrent,
                info,
                hash_id,
                torrent_state: Arc::new(Mutex::new(TorrentProcessState::Init)),
                announce_state: Arc::new(Mutex::new(AnnounceState::Idle)),
                stats: Arc::new(Mutex::new(TorrentProcessStats {
                    downloaded: 0,
                    uploaded: 0,
                    left,
                })),
                blocks_downloading: Arc::new(Mutex::new(HashMap::new())),
                torrent_storage: RwLock::new(TorrentStorage {
                    pieces,
                    peers: vec![],
                }),
            });

            processes.push(process.clone());

            Ok(process)
        }
    */

    pub async fn download<P: AsRef<Path>>(&self, torrent_file: P) -> Result<(), RustorrentError> {
        let config = &self.settings.config;

        let listen = config
            .listen
            .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));

        let addr = SocketAddr::new(listen.into(), config.port);

        let (mut download_events_sender, download_events_receiver) =
            mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let download_events = download_events_loop(
            self.settings.clone(),
            download_events_sender.clone(),
            download_events_receiver,
        );

        download_events_sender
            .send(RustorrentEvent::AddTorrent(torrent_file.as_ref().into()))
            .await?;

        let accept_incoming_connections =
            accept_connections_loop(self.settings.clone(), addr, download_events_sender.clone());

        if let Err(err) = try_join(accept_incoming_connections, download_events).await {
            return Err(err);
        }

        Ok(())
    }
}

fn spawn_and_log_error<F>(f: F) -> tokio::task::JoinHandle<()>
where
    F: Future<Output = Result<(), RustorrentError>> + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = f.await {
            error!("{}", e)
        }
    })
}

async fn accept_connections_loop(
    settings: Arc<Settings>,
    addr: SocketAddr,
    sender: Sender<RustorrentEvent>,
) -> Result<(), RustorrentError> {
    debug!("listening on: {}", &addr);
    let mut listener = TcpListener::bind(addr).await?;

    loop {
        let (socket, _) = listener.accept().await?;
        let _ = spawn_and_log_error(peer_connection(settings.clone(), socket, sender.clone()));
    }
}

async fn peer_connection(
    settings: Arc<Settings>,
    mut socket: TcpStream,
    mut sender: Sender<RustorrentEvent>,
) -> Result<(), RustorrentError> {
    let mut handshake_request = vec![0u8; 68];

    socket.read_exact(&mut handshake_request).await?;

    let handshake_request: Handshake = handshake_request.try_into()?;

    let (handshake_sender, handshake_receiver) = oneshot::channel();

    sender
        .send(RustorrentEvent::TorrentHandshake {
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
            return Err(RustorrentError::PeerHandshakeFailure);
        }
    };

    socket.write_all(&torrent_process.handshake).await?;

    debug!("handshake done, connected with peer");

    let (wtransport, mut rtransport) = Framed::new(socket, MessageCodec).split();

    let receive_task = async move {
        while let Some(Ok(message)) = rtransport.next().await {
            debug!("received peer message: {}", message);
        }

        debug!("peer connection receive exit");

        Ok::<(), RustorrentError>(())
    };

    receive_task.await?;

    debug!("peer connection exit");

    Ok(())
}

async fn download_events_loop(
    settings: Arc<Settings>,
    mut sender: Sender<RustorrentEvent>,
    mut events: Receiver<RustorrentEvent>,
) -> Result<(), RustorrentError> {
    let mut torrents = vec![];

    while let Some(event) = events.next().await {
        match event {
            RustorrentEvent::AddTorrent(filename) => {
                debug!("we need to download {:?}", filename);
                let torrent = parse_torrent(&filename)?;
                let hash_id = torrent.info_sha1_hash();
                let info = torrent.info()?;

                let mut handshake = vec![];
                handshake.extend_from_slice(&crate::types::HANDSHAKE_PREFIX);
                handshake.extend_from_slice(&hash_id);
                handshake.extend_from_slice(&PEER_ID);

                let (broker_sender, broker_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

                let torrent_process = Arc::new(TorrentProcess {
                    info,
                    hash_id,
                    torrent,
                    handshake,
                    broker_sender,
                });

                torrents.push(torrent_process.clone());

                let _ = spawn_and_log_error(download_torrent(
                    settings.clone(),
                    torrent_process,
                    broker_receiver,
                ));
            }
            RustorrentEvent::TorrentHandshake {
                handshake_request,
                handshake_sender,
            } => {
                debug!(
                    "searching for matching torrent handshake: {:?}",
                    handshake_request
                );

                let hash_id = handshake_request.info_hash;

                if let Err(_) =
                    handshake_sender.send(torrents.iter().find(|x| x.hash_id == hash_id).cloned())
                {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
        }
    }

    Ok(())
}

enum DownloadTorrentEvent {
    Announce(Vec<Peer>),
    PeerAnnounced(Peer),
    PeerConnected(Uuid, TcpStream),
    PeerConnectFailed(Uuid),
    PeerDisconnect(Uuid),
    PeerPieces(Uuid, Vec<u8>),
}

async fn announce_loop(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
) -> Result<(), RustorrentError> {
    loop {
        let announce_url = &torrent_process.torrent.announce_url;

        let client: Client<_> = Client::new();

        let left = torrent_process.info.len();
        let config = &settings.config;
        let mut url = {
            format!(
                "{}?info_hash={}&peer_id={}&left={}&port={}",
                announce_url,
                url_encode(&torrent_process.hash_id[..]),
                url_encode(&PEER_ID[..]),
                left,
                config.port,
            )
        };

        if let Some(compact) = config.compact {
            url += &format!("&compact={}", if compact { 1 } else { 0 });
        }

        let uri = url.parse()?;
        let res = client.get(uri).await;

        debug!("Got tracker announce from: {}", url);

        let result = match res {
            Ok(result) if result.status().is_success() => result,
            Ok(bad_result) => {
                error!(
                    "Bad response from tracker: {:?}, retry in 5 seconds...",
                    bad_result
                );
                delay_for(Duration::from_secs(5)).await;
                continue;
            }
            Err(err) => {
                error!("Failure {}, retry in 5 seconds", err);
                delay_for(Duration::from_secs(5)).await;
                continue;
            }
        };

        let mut announce_data = result.into_body();

        let mut announce_bytes = vec![];

        while let Some(chunk) = announce_data.data().await {
            announce_bytes.append(&mut chunk?.to_vec());
        }

        let tracker_announce: Result<TrackerAnnounce, _> = announce_bytes.try_into();

        let interval_to_query_tracker = match tracker_announce {
            Ok(tracker_announce) => {
                let interval_to_reannounce = tracker_announce.interval.try_into()?;

                debug!("Tracker announce: {:?}", tracker_announce);

                torrent_process
                    .broker_sender
                    .clone()
                    .send(DownloadTorrentEvent::Announce(tracker_announce.peers))
                    .await?;
                Duration::from_secs(interval_to_reannounce)
            }

            Err(err) => {
                error!("Failure {}, retry in 5 seconds", err);
                Duration::from_secs(5)
            }
        };

        debug!("query tracker in {:?}", interval_to_query_tracker);

        delay_for(interval_to_query_tracker).await;
    }
}

async fn download_torrent(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    mut broker_receiver: Receiver<DownloadTorrentEvent>,
) -> Result<(), RustorrentError> {
    let mut torrent_storage = TorrentStorage {
        downloaded: vec![],
        pieces: vec![],
    };

    let (abort_handle, abort_registration) = AbortHandle::new_pair();

    let announce_loop = Abortable::new(
        announce_loop(settings.clone(), torrent_process.clone()),
        abort_registration,
    )
    .map_err(|e| e.into());

    let mut peer_states = HashMap::new();

    let download_events_loop = async move {
        while let Some(event) = broker_receiver.next().await {
            debug!("received event");
            match event {
                DownloadTorrentEvent::Announce(peers) => {
                    debug!("we got announce, what now?");
                    spawn_and_log_error(process_announce(
                        settings.clone(),
                        torrent_process.clone(),
                        peers,
                    ));
                }
                DownloadTorrentEvent::PeerAnnounced(peer) => {
                    debug!("peer announced: {:?}", peer);
                    process_peer_announced(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer,
                    )
                    .await?;
                }
                DownloadTorrentEvent::PeerDisconnect(peer_id) => {
                    if let Some(_peer_state) = peer_states.remove(&peer_id) {
                        debug!("removed peer {} due to disconnect", peer_id);
                    }
                }
                DownloadTorrentEvent::PeerConnectFailed(peer_id) => {
                    if let Some(_peer_state) = peer_states.remove(&peer_id) {
                        debug!("removed peer {} due to connection failure", peer_id);
                    }
                }
                DownloadTorrentEvent::PeerConnected(peer_id, stream) => {
                    debug!("peer connected: {}", peer_id);
                    process_peer_connected(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                        stream,
                    )
                    .await?;
                }
                DownloadTorrentEvent::PeerPieces(peer_id, pieces) => {
                    debug!("peer pieces {}", peer_id);
                    process_peer_pieces(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                        pieces,
                        &mut torrent_storage,
                    )
                    .await?;
                }
            }
        }

        abort_handle.abort();

        debug!("download events loop is done");

        Ok::<(), RustorrentError>(())
    };

    match try_join!(announce_loop, download_events_loop) {
        Ok(_) | Err(RustorrentError::Aborted) => debug!("download torrent is done"),
        Err(e) => error!("download torrent finished with failure: {}", e),
    }

    Ok(())
}

async fn process_announce(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peers: Vec<Peer>,
) -> Result<(), RustorrentError> {
    let mut download_torrent_broker_sender = torrent_process.broker_sender.clone();

    for peer in peers {
        download_torrent_broker_sender
            .send(DownloadTorrentEvent::PeerAnnounced(peer))
            .await?;
    }

    Ok(())
}

async fn process_peer_announced(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer: Peer,
) -> Result<(), RustorrentError> {
    let mut peer_states_iter = peer_states.iter_mut();
    if let Some((peer_id, existing_peer)) = peer_states_iter.find(|x| x.1.peer == peer) {
        match existing_peer.state {
            TorrentPeerState::Idle => {
                let handler = spawn_and_log_error(connect_to_peer(
                    settings,
                    torrent_process,
                    peer_id.clone(),
                    peer,
                ));
                existing_peer.state = TorrentPeerState::Connecting(handler);
            }
            TorrentPeerState::Connected { .. } => {
                existing_peer.announce_count += 1;
            }
            _ => (),
        }
    } else {
        let peer_id = Uuid::new_v4();
        peer_states.insert(
            peer_id,
            PeerState {
                peer: peer.clone(),
                state: TorrentPeerState::Connecting(spawn_and_log_error(connect_to_peer(
                    settings,
                    torrent_process,
                    peer_id,
                    peer,
                ))),
                announce_count: 0,
            },
        );
    };

    Ok(())
}

async fn connect_to_peer(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_id: Uuid,
    peer: Peer,
) -> Result<(), RustorrentError> {
    let socket_addr = SocketAddr::new(peer.ip, peer.port);
    let mut stream = TcpStream::connect(socket_addr).await?;

    stream.write_all(&torrent_process.handshake).await?;

    let mut handshake_reply = vec![0u8; 68];

    stream.read_exact(&mut handshake_reply).await?;

    let handshake_reply: Handshake = handshake_reply.try_into()?;

    if handshake_reply.info_hash != torrent_process.hash_id {
        error!("Peer {:?}: hash is wrong. Disconnect.", peer);
        torrent_process
            .broker_sender
            .clone()
            .send(DownloadTorrentEvent::PeerConnectFailed(peer_id))
            .await?;
        return Ok(());
    }

    torrent_process
        .broker_sender
        .clone()
        .send(DownloadTorrentEvent::PeerConnected(peer_id, stream))
        .await?;

    Ok(())
}

async fn process_peer_connected(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
    stream: TcpStream,
) -> Result<(), RustorrentError> {
    debug!("peer connection initiated: {:?}", peer_id);

    if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        let (sender, receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let _ = spawn_and_log_error(peer_loop(
            settings,
            torrent_process,
            peer_id,
            sender.clone(),
            receiver,
            stream,
        ));

        existing_peer.state = TorrentPeerState::Connected {
            chocked: true,
            interested: false,
            downloading_piece: None,
            pieces: vec![],
            sender,
        };
    }

    Ok(())
}

async fn peer_loop(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_id: Uuid,
    mut sender: Sender<PeerMessage>,
    mut receiver: Receiver<PeerMessage>,
    stream: TcpStream,
) -> Result<(), RustorrentError> {
    let (mut wtransport, mut rtransport) = Framed::new(stream, MessageCodec).split();

    let mut broker_sender = torrent_process.broker_sender.clone();

    let mut command_loop_broker_sender = broker_sender.clone();

    let command_loop = async move {
        let mut message_count = 0;
        while let Some(message) = receiver.next().await {
            debug!("peer loop received message: {:?}", message);
            match message {
                PeerMessage::Download(piece) => {
                    info!("we got far, let's download now piece: {}", piece);
                }
                PeerMessage::Disconnect => break,
                PeerMessage::Message(message) => {
                    message_count += 1;
                    match message {
                        Message::Bitfield(pieces) => {
                            if message_count != 1 {
                                error!("wrong message sequence for peer {}: bitfield message must be first message", peer_id);
                                break;
                            }
                            command_loop_broker_sender
                                .send(DownloadTorrentEvent::PeerPieces(peer_id, pieces))
                                .await?;
                        }
                        _ => (),
                    }
                }
            }
        }

        debug!("peer loop command exit");

        wtransport.close().await?;

        Ok::<(), RustorrentError>(())
    };

    let receive_loop = async move {
        while let Some(Ok(message)) = rtransport.next().await {
            sender.send(PeerMessage::Message(message)).await?;
        }

        debug!("peer loop receive exit");

        if let Err(err) = sender.send(PeerMessage::Disconnect).await {
            error!(
                "cannot send disconnect message to peer {}: {}",
                peer_id, err
            );
        }

        Ok::<(), RustorrentError>(())
    };

    let _ = try_join!(command_loop, receive_loop)?;

    broker_sender
        .send(DownloadTorrentEvent::PeerDisconnect(peer_id))
        .await?;

    debug!("peer loop exit");

    Ok(())
}

async fn process_peer_pieces(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
    peer_pieces: Vec<u8>,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    debug!("peer connection initiated: {:?}", peer_id);

    let new_pieces = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        match &mut existing_peer.state {
            TorrentPeerState::Connected { pieces, .. } => {
                collect_pieces_and_update(pieces, &peer_pieces, &storage.downloaded)
            }
            TorrentPeerState::Idle | TorrentPeerState::Connecting(_) => {
                error!(
                    "cannot process peer pieces: wrong state: {:?}",
                    existing_peer.state
                );
                vec![]
            }
        }
    } else {
        vec![]
    };

    for new_piece in new_pieces {
        let any_peer_downloading = peer_states.values().any(|x| match x.state {
            TorrentPeerState::Connected {
                downloading_piece, ..
            } => downloading_piece == Some(new_piece),
            _ => false,
        });

        if !any_peer_downloading {
            if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
                if let TorrentPeerState::Connected {
                    ref mut downloading_piece,
                    ref mut sender,
                    ..
                } = existing_peer.state
                {
                    if downloading_piece.is_none() {
                        *downloading_piece = Some(new_piece);
                        sender.send(PeerMessage::Download(new_piece)).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn collect_pieces_and_update(
    current_pieces: &mut Vec<u8>,
    new_pieces: &[u8],
    downloaded_pieces: &[u8],
) -> Vec<usize> {
    let mut pieces = vec![];
    while current_pieces.len() < new_pieces.len() {
        current_pieces.push(0);
    }
    for (i, (a, &b)) in current_pieces.iter_mut().zip(new_pieces).enumerate() {
        let new = b & !*a;

        *a |= new;

        let new = if let Some(d) = downloaded_pieces.get(i) {
            b & !d
        } else {
            b
        };

        for j in 0..8 {
            if new & (0b10000000 >> j) != 0 {
                pieces.push(i * 8 + j);
            }
        }
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_collect_pieces_and_update() {
        let mut current_pieces = vec![];

        let result = collect_pieces_and_update(&mut current_pieces, &[192], &[]);
        assert_eq!(result, vec![0, 1]);
        assert_eq!(current_pieces, vec![192]);

        let mut current_pieces = vec![192];

        let result = collect_pieces_and_update(&mut current_pieces, &[192], &[192]);
        assert_eq!(result, vec![]);
        assert_eq!(current_pieces, vec![192]);

        let mut current_pieces = vec![];

        let result = collect_pieces_and_update(&mut current_pieces, &[192, 192], &[]);
        assert_eq!(result, vec![0, 1, 8, 9]);
        assert_eq!(current_pieces, vec![192, 192]);

        let mut current_pieces = vec![];

        let result = collect_pieces_and_update(&mut current_pieces, &[0b10101010], &[0b01010101]);
        assert_eq!(result, vec![0, 2, 4, 6]);
        assert_eq!(current_pieces, vec![0b10101010]);

        let mut current_pieces = vec![];

        let result = collect_pieces_and_update(&mut current_pieces, &[0b10101010], &[0b11010101]);
        assert_eq!(result, vec![2, 4, 6]);
        assert_eq!(current_pieces, vec![0b10101010]);
    }
}
