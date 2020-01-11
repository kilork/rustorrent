use super::*;
use crate::{errors::RustorrentError, types::torrent::parse_torrent, PEER_ID};

use crate::{
    types::{
        info::TorrentInfo,
        message::{Message, MessageCodec},
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
    pub(crate) torrent: Torrent,
    pub(crate) info: TorrentInfo,
    pub(crate) hash_id: [u8; SHA1_SIZE],
    pub(crate) handshake: Vec<u8>,
    broker_sender: Sender<DownloadTorrentEvent>,
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
    Have(usize),
    Bitfield(Vec<u8>),
}

impl RustorrentApp {
    pub fn new(settings: Settings) -> Self {
        let settings = Arc::new(settings);
        Self { settings }
    }

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

    torrent_process
        .broker_sender
        .clone()
        .send(DownloadTorrentEvent::PeerForwarded(socket))
        .await?;

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
                debug!("searching for matching torrent handshake");

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

#[derive(Debug)]
enum DownloadTorrentEvent {
    Announce(Vec<Peer>),
    PeerAnnounced(Peer),
    PeerConnected(Uuid, TcpStream),
    PeerForwarded(TcpStream),
    PeerConnectFailed(Uuid),
    PeerDisconnect(Uuid),
    PeerPieces(Uuid, Vec<u8>),
    PeerPiece(Uuid, usize),
    PeerUnchoke(Uuid),
    PeerPieceDownloaded(Uuid, Vec<u8>),
}

impl Display for DownloadTorrentEvent {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            DownloadTorrentEvent::PeerPieceDownloaded(uuid, data) => {
                write!(f, "PeerPieceDownloaded({}, [{}])", uuid, data.len())
            }
            _ => write!(f, "{:?}", self),
        }
    }
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
    let mut torrent_storage = TorrentStorage::new(torrent_process.clone());

    let (abort_handle, abort_registration) = AbortHandle::new_pair();

    let announce_loop = Abortable::new(
        announce_loop(settings.clone(), torrent_process.clone()),
        abort_registration,
    )
    .map_err(|e| e.into());

    let mut peer_states = HashMap::new();

    let download_events_loop = async move {
        while let Some(event) = broker_receiver.next().await {
            debug!("received event: {}", event);
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
                        debug!("[{}] removed peer due to disconnect", peer_id);
                    }
                }
                DownloadTorrentEvent::PeerConnectFailed(peer_id) => {
                    if let Some(_peer_state) = peer_states.remove(&peer_id) {
                        debug!("[{}] removed peer due to connection failure", peer_id);
                    }
                }
                DownloadTorrentEvent::PeerForwarded(stream) => {
                    debug!("peer forwarded");
                    process_peer_forwarded(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        stream,
                        &mut torrent_storage,
                    )
                    .await?;
                }
                DownloadTorrentEvent::PeerConnected(peer_id, stream) => {
                    debug!("[{}] peer connected", peer_id);
                    process_peer_connected(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                        stream,
                    )
                    .await?;
                }
                DownloadTorrentEvent::PeerPiece(peer_id, piece) => {
                    debug!("[{}] peer piece: {}", peer_id, piece);
                    process_peer_piece(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                        piece,
                        &mut torrent_storage,
                    )
                    .await?;
                }
                DownloadTorrentEvent::PeerPieces(peer_id, pieces) => {
                    debug!("[{}] peer pieces", peer_id);
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
                DownloadTorrentEvent::PeerUnchoke(peer_id) => {
                    debug!("[{}] peer unchoke", peer_id);
                    process_peer_unchoke(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                    )
                    .await?;
                }
                DownloadTorrentEvent::PeerPieceDownloaded(peer_id, piece) => {
                    debug!("[{}] downloaded piece for peer", peer_id);
                    process_peer_piece_downloaded(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                        piece,
                        &mut torrent_storage,
                    )
                    .await?;

                    if torrent_storage.receiver.borrow().pieces_left == 0 {
                        debug!(
                            "torrent downloaded, hash: {}",
                            percent_encode(&torrent_process.hash_id, NON_ALPHANUMERIC)
                        );
                    }
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
        error!("[{}] peer {:?}: hash is wrong. Disconnect.", peer_id, peer);
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

async fn process_peer_forwarded(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    stream: TcpStream,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    let peer_id = Uuid::new_v4();
    debug!("[{}] peer connection forwarded", peer_id);

    let peer_addr = stream.peer_addr()?;

    let peer: Peer = peer_addr.into();

    let (mut sender, receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

    peer_states.insert(
        peer_id,
        PeerState {
            peer: peer.clone(),
            state: TorrentPeerState::Connected {
                chocked: true,
                interested: false,
                downloading_piece: None,
                pieces: vec![],
                sender: sender.clone(),
            },
            announce_count: 0,
        },
    );

    {
        let downloaded = storage.receiver.borrow().downloaded.clone();
        if !downloaded.is_empty() {
            sender.send(PeerMessage::Bitfield(downloaded)).await?;
        }
    }

    let _ = spawn_and_log_error(peer_loop(
        settings,
        torrent_process,
        peer_id,
        sender,
        receiver,
        stream,
    ));

    Ok(())
}

async fn process_peer_connected(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
    stream: TcpStream,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer connection initiated", peer_id);

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

fn request_message(buffer: &[u8], piece: usize, piece_length: usize) -> Message {
    let index = piece as u32;
    let begin = buffer.len() as u32;
    let length = if piece_length - buffer.len() < BLOCK_SIZE {
        piece_length - buffer.len()
    } else {
        BLOCK_SIZE
    } as u32;
    Message::Request {
        index,
        begin,
        length,
    }
}

async fn peer_loop(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_id: Uuid,
    mut sender: Sender<PeerMessage>,
    mut receiver: Receiver<PeerMessage>,
    stream: TcpStream,
) -> Result<(), RustorrentError> {
    let (wtransport, mut rtransport) = Framed::new(stream, MessageCodec).split();

    let mut broker_sender = torrent_process.broker_sender.clone();

    let command_loop_broker_sender = broker_sender.clone();

    let command_loop = async move {
        let mut processor = PeerLoopMessage {
            peer_id,
            command_loop_broker_sender,
            torrent_process: torrent_process.clone(),
            chocked: true,
            interested: false,
            message_count: 0,
            downloading: None,
            piece_length: 0,
            torrent_piece: None,
            wtransport,
        };

        while let Some(message) = receiver.next().await {
            debug!("[{}] peer loop received message", peer_id);
            match message {
                PeerMessage::Bitfield(pieces) => {
                    processor.wtransport.send(Message::Bitfield(pieces)).await?;
                }
                PeerMessage::Have(piece) => {
                    let piece_index = piece as u32;
                    processor
                        .wtransport
                        .send(Message::Have { piece_index })
                        .await?;
                }
                PeerMessage::Download(piece) => {
                    debug!("[{}] download now piece: {}", peer_id, piece);
                    processor.piece_length = torrent_process.info.sizes(piece).0;
                    processor.downloading = Some(piece);
                    processor.torrent_piece = Some(Vec::with_capacity(processor.piece_length));

                    if processor.chocked {
                        debug!("[{}] send interested message", peer_id);
                        processor.wtransport.send(Message::Interested).await?;
                    } else if let Some(ref torrent_peer_piece) = processor.torrent_piece {
                        processor
                            .wtransport
                            .send(request_message(
                                torrent_peer_piece,
                                piece,
                                processor.piece_length,
                            ))
                            .await?;
                    }
                }
                PeerMessage::Disconnect => break,
                PeerMessage::Message(message) => {
                    if processor.peer_loop_message(message).await? {
                        break;
                    }
                }
            }
        }

        debug!("[{}] peer loop command exit", peer_id);

        processor.wtransport.close().await?;

        Ok::<(), RustorrentError>(())
    };

    let receive_loop = async move {
        while let Some(Ok(message)) = rtransport.next().await {
            sender.send(PeerMessage::Message(message)).await?;
        }

        debug!("[{}] peer loop receive exit", peer_id);

        if let Err(err) = sender.send(PeerMessage::Disconnect).await {
            error!(
                "[{}] cannot send disconnect message to peer: {}",
                peer_id, err
            );
        }

        Ok::<(), RustorrentError>(())
    };

    let _ = try_join!(command_loop, receive_loop)?;

    broker_sender
        .send(DownloadTorrentEvent::PeerDisconnect(peer_id))
        .await?;

    debug!("[{}] peer loop exit", peer_id);

    Ok(())
}

struct PeerLoopMessage {
    torrent_process: Arc<TorrentProcess>,
    message_count: usize,
    chocked: bool,
    interested: bool,
    peer_id: Uuid,
    command_loop_broker_sender: Sender<DownloadTorrentEvent>,
    downloading: Option<usize>,
    torrent_piece: Option<Vec<u8>>,
    piece_length: usize,
    wtransport: SplitSink<Framed<TcpStream, MessageCodec>, Message>,
}

impl PeerLoopMessage {
    async fn bitfield(&mut self, pieces: Vec<u8>) -> Result<bool, RustorrentError> {
        let peer_id = self.peer_id;
        if self.message_count != 1 {
            error!(
                "[{}] wrong message sequence for peer: bitfield message must be first message",
                peer_id
            );
            return Ok(true);
        }
        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerPieces(peer_id, pieces))
            .await?;

        Ok(false)
    }

    async fn have(&mut self, piece_index: usize) -> Result<bool, RustorrentError> {
        let peer_id = self.peer_id;

        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerPiece(peer_id, piece_index))
            .await?;

        Ok(false)
    }

    async fn unchoke(&mut self) -> Result<bool, RustorrentError> {
        self.chocked = false;
        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerUnchoke(self.peer_id))
            .await?;

        if let Some(piece) = self.downloading {
            if let Some(ref torrent_peer_piece) = self.torrent_piece {
                self.wtransport
                    .send(request_message(
                        torrent_peer_piece,
                        piece,
                        self.piece_length,
                    ))
                    .await?;
            }
        }

        Ok(false)
    }

    async fn piece(
        &mut self,
        index: u32,
        begin: u32,
        block: Vec<u8>,
    ) -> Result<bool, RustorrentError> {
        let peer_id = self.peer_id;

        if let Some(piece) = self.downloading {
            if piece as u32 != index {
                error!(
                    "[{}] abnormal piece message {} for peer, expected {}",
                    peer_id, index, piece
                );
                return Ok(false);
            }
            if let Some(ref mut torrent_peer_piece) = self.torrent_piece {
                if torrent_peer_piece.len() != begin as usize {
                    error!(
                            "[{}] abnormal piece message for peer piece {}, expected begin {} but got {}",
                            peer_id, piece, torrent_peer_piece.len(), begin,
                        );
                    return Ok(false);
                }

                torrent_peer_piece.extend(block);

                if self.piece_length > torrent_peer_piece.len() {
                    self.wtransport
                        .send(request_message(
                            torrent_peer_piece,
                            piece,
                            self.piece_length,
                        ))
                        .await?;
                } else if torrent_peer_piece.len() == self.piece_length {
                    let control_piece = &self.torrent_process.info.pieces[piece];

                    let sha1: types::info::Piece =
                        Sha1::digest(torrent_peer_piece.as_slice())[..].try_into()?;
                    if sha1 != *control_piece {
                        error!("[{}] piece sha1 failure", peer_id);
                    }

                    self.downloading = None;
                    self.command_loop_broker_sender
                        .send(DownloadTorrentEvent::PeerPieceDownloaded(
                            peer_id,
                            self.torrent_piece.take().unwrap(),
                        ))
                        .await?;
                } else {
                    error!(
                        "[{}] wrong piece length: {} {}",
                        peer_id,
                        piece,
                        torrent_peer_piece.len()
                    );
                    return Ok(false);
                }
            }
        } else {
            error!("[{}] abnormal piece message {} for peer", peer_id, index);
        }

        Ok(false)
    }

    async fn peer_loop_message(&mut self, message: Message) -> Result<bool, RustorrentError> {
        let peer_id = self.peer_id;
        debug!("[{}] message {}", peer_id, message);
        self.message_count += 1;
        match message {
            Message::Bitfield(pieces) => {
                return self.bitfield(pieces).await;
            }
            Message::Have { piece_index } => {
                return self.have(piece_index as usize).await;
            }
            Message::Unchoke => {
                return self.unchoke().await;
            }
            Message::Piece {
                index,
                begin,
                block,
            } => {
                return self.piece(index, begin, block).await;
            }
            _ => debug!("[{}] unhandled message: {}", peer_id, message),
        }

        Ok(false)
    }
}

async fn process_peer_piece(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
    peer_piece: usize,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer piece", peer_id);

    let new_pieces = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        match existing_peer.state {
            TorrentPeerState::Connected { .. } => {
                let mut downloadable = vec![];
                let (index, bit) = crate::messages::index_in_bitarray(peer_piece);
                match_pieces(
                    &mut downloadable,
                    &storage.receiver.borrow().downloaded,
                    index,
                    bit,
                );
                downloadable
            }
            TorrentPeerState::Idle | TorrentPeerState::Connecting(_) => {
                error!(
                    "[{}] cannot process peer piece: wrong state: {:?}",
                    peer_id, existing_peer.state
                );
                vec![]
            }
        }
    } else {
        vec![]
    };

    select_new_peer(&new_pieces, peer_states, peer_id).await?;

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
    debug!("[{}] process peer pieces", peer_id);

    let new_pieces = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        match &mut existing_peer.state {
            TorrentPeerState::Connected { pieces, .. } => collect_pieces_and_update(
                pieces,
                &peer_pieces,
                &storage.receiver.borrow().downloaded,
            ),
            TorrentPeerState::Idle | TorrentPeerState::Connecting(_) => {
                error!(
                    "[{}] cannot process peer pieces: wrong state: {:?}",
                    peer_id, existing_peer.state
                );
                vec![]
            }
        }
    } else {
        vec![]
    };

    select_new_peer(&new_pieces, peer_states, peer_id).await?;

    Ok(())
}

async fn select_new_peer(
    new_pieces: &[usize],
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
) -> Result<(), RustorrentError> {
    for &new_piece in new_pieces {
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

async fn process_peer_piece_downloaded(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
    piece: Vec<u8>,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer piece downloaded", peer_id);

    let (index, new_pieces) = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        if let TorrentPeerState::Connected {
            ref pieces,
            ref mut downloading_piece,
            ref mut sender,
            ..
        } = existing_peer.state
        {
            if let Some(index) = downloading_piece.take() {
                storage.save(index, piece).await?;

                let mut downloadable = vec![];
                for (i, &a) in pieces.iter().enumerate() {
                    match_pieces(
                        &mut downloadable,
                        &storage.receiver.borrow().downloaded,
                        i,
                        a,
                    );
                }
                (index, downloadable)
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    } else {
        return Ok(());
    };

    for (_, peer_state) in peer_states.iter_mut().filter(|(&key, _)| key != peer_id) {
        if let TorrentPeerState::Connected { ref mut sender, .. } = peer_state.state {
            sender.send(PeerMessage::Have(index)).await?;
        }
    }

    select_new_peer(&new_pieces, peer_states, peer_id).await?;

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

        match_pieces(&mut pieces, downloaded_pieces, i, b);
    }
    pieces
}

fn match_pieces(pieces: &mut Vec<usize>, downloaded_pieces: &[u8], i: usize, a: u8) {
    let new = if let Some(d) = downloaded_pieces.get(i) {
        a & !d
    } else {
        a
    };

    for j in 0..8 {
        if new & (0b10000000 >> j) != 0 {
            pieces.push(i * 8 + j);
        }
    }
}

async fn process_peer_unchoke(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
) -> Result<(), RustorrentError> {
    debug!("[{}] process peer unchoke", peer_id);

    if let Some(TorrentPeerState::Connected {
        ref mut chocked, ..
    }) = peer_states.get_mut(&peer_id).map(|x| &mut x.state)
    {
        *chocked = false;
    }

    Ok(())
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

    fn test_settings() -> Arc<Settings> {
        Arc::new(Default::default())
    }

    #[tokio::test]
    async fn check_process_peer_pieces() {}
}
