use super::*;
use crate::{errors::RustorrentError, types::torrent::parse_torrent, PEER_ID};

use crate::{
    messages::{bit_by_index, index_in_bitarray},
    types::{
        info::TorrentInfo,
        message::{Message, MessageCodec},
        peer::{Handshake, Peer},
        torrent::Torrent,
        Settings,
    },
    SHA1_SIZE,
};

pub struct RustorrentApp {
    pub settings: Arc<Settings>,
}

#[derive(Debug)]
pub struct TorrentProcess {
    pub id: usize,
    pub filename: String,
    pub(crate) torrent: Torrent,
    pub info: TorrentInfo,
    pub(crate) hash_id: [u8; SHA1_SIZE],
    pub(crate) handshake: Vec<u8>,
    pub(crate) broker_sender: Sender<DownloadTorrentEvent>,
}

#[derive(Debug)]
enum TorrentPeerState {
    Idle,
    Connecting(JoinHandle<()>),
    Connected {
        chocked: bool,
        interested: bool,
        downloading_piece: Option<usize>,
        downloading_since: Option<Instant>,
        downloaded: usize,
        uploaded: usize,
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

pub enum RequestResponse<T, R> {
    RequestOnly(T),
    ResponseOnly(oneshot::Sender<R>),
    Full {
        request: T,
        response: oneshot::Sender<R>,
    },
}

impl<T, R> RequestResponse<T, R> {
    pub fn request(&self) -> Option<&T> {
        match self {
            RequestResponse::RequestOnly(request) | RequestResponse::Full { request, .. } => {
                Some(request)
            }
            RequestResponse::ResponseOnly(_) => None,
        }
    }

    pub fn response(self, result: R) -> Result<(), RustorrentError> {
        match self {
            RequestResponse::ResponseOnly(response) | RequestResponse::Full { response, .. } => {
                response
                    .send(result)
                    .map_err(|_| RustorrentError::FailureReason("Cannot send response".into()))
            }
            RequestResponse::RequestOnly(_) => Ok(()),
        }
    }
}

pub enum RustorrentCommand {
    AddTorrent(
        RequestResponse<Vec<u8>, Result<Arc<TorrentProcess>, RustorrentError>>,
        String,
    ),
    TorrentHandshake {
        handshake_request: Handshake,
        handshake_sender: oneshot::Sender<Option<Arc<TorrentProcess>>>,
    },
    TorrentList {
        sender: oneshot::Sender<Vec<Arc<TorrentProcess>>>,
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
    Cancel,
    Message(Message),
    Download(usize),
    Have(usize),
    Bitfield(Vec<u8>),
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
}

enum TorrentDownloadMode {
    Normal,
    Final,
}

impl RustorrentApp {
    pub fn new(settings: Settings) -> Self {
        let settings = Arc::new(settings);
        Self { settings }
    }

    pub async fn processing_loop(
        &self,
        sender: Sender<RustorrentCommand>,
        receiver: Receiver<RustorrentCommand>,
    ) -> Result<(), RustorrentError> {
        let config = &self.settings.config;

        let listen = config
            .listen
            .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));

        let addr = SocketAddr::new(listen.into(), config.port);

        let download_events = download_events_loop(self.settings.clone(), sender.clone(), receiver);

        let accept_incoming_connections =
            accept_connections_loop(self.settings.clone(), addr, sender.clone());

        if let Err(err) = try_join(accept_incoming_connections, download_events).await {
            return Err(err);
        }

        Ok(())
    }

    pub async fn download<P: AsRef<Path>>(&self, torrent_file: P) -> Result<(), RustorrentError> {
        let (mut download_events_sender, download_events_receiver) =
            mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let buf = std::fs::read(torrent_file.as_ref())?;

        download_events_sender
            .send(RustorrentCommand::AddTorrent(
                RequestResponse::RequestOnly(buf),
                torrent_file
                    .as_ref()
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap_or_default()
                    .into(),
            ))
            .await?;

        self.processing_loop(download_events_sender, download_events_receiver)
            .await
    }
}

fn spawn_and_log_error<F, M>(f: F, message: M) -> tokio::task::JoinHandle<()>
where
    F: Future<Output = Result<(), RustorrentError>> + Send + 'static,
    M: Fn() -> String + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = f.await {
            error!("{}: {}", message(), e)
        }
    })
}

async fn accept_connections_loop(
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

async fn peer_connection(
    settings: Arc<Settings>,
    mut socket: TcpStream,
    mut sender: Sender<RustorrentCommand>,
) -> Result<(), RustorrentError> {
    let mut handshake_request = vec![0u8; 68];

    socket.read_exact(&mut handshake_request).await?;

    let handshake_request: Handshake = handshake_request.try_into()?;

    let (handshake_sender, handshake_receiver) = oneshot::channel();

    sender
        .send(RustorrentCommand::TorrentHandshake {
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
    mut sender: Sender<RustorrentCommand>,
    mut events: Receiver<RustorrentCommand>,
) -> Result<(), RustorrentError> {
    let mut torrents = vec![];
    let mut id = 0;

    while let Some(event) = events.next().await {
        match event {
            RustorrentCommand::AddTorrent(request_response, filename) => {
                debug!("we need to download {:?}", filename);
                if let Some(request) = request_response.request() {
                    let torrent = parse_torrent(request)?;
                    let hash_id = torrent.info_sha1_hash();
                    let info = torrent.info()?;

                    debug!("torrent size: {}", info.len());
                    debug!("piece length: {}", info.piece_length);
                    debug!("total pieces: {}", info.pieces.len());

                    let mut handshake = vec![];
                    handshake.extend_from_slice(&crate::types::HANDSHAKE_PREFIX);
                    handshake.extend_from_slice(&hash_id);
                    handshake.extend_from_slice(&PEER_ID);

                    let (broker_sender, broker_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);
                    id += 1;
                    let torrent_process = Arc::new(TorrentProcess {
                        id,
                        filename,
                        info,
                        hash_id,
                        torrent,
                        handshake,
                        broker_sender,
                    });

                    torrents.push(torrent_process.clone());

                    let _ = spawn_and_log_error(
                        download_torrent(
                            settings.clone(),
                            torrent_process.clone(),
                            broker_receiver,
                        ),
                        || format!("download_events_loop: add torrent failed"),
                    );

                    if let Err(err) = request_response.response(Ok(torrent_process)) {
                        error!("cannot send response for add torrent: {}", err);
                    }
                }
            }
            RustorrentCommand::TorrentHandshake {
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
            RustorrentCommand::TorrentList { sender } => {
                debug!("collecting torrent list");
                if let Err(_) = sender.send(torrents.iter().cloned().collect()) {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
        }
    }

    debug!("download_events_loop done");

    Ok(())
}

#[derive(Debug)]
pub(crate) enum DownloadTorrentEvent {
    Announce(Vec<Peer>),
    PeerAnnounced(Peer),
    PeerConnected(Uuid, TcpStream),
    PeerForwarded(TcpStream),
    PeerConnectFailed(Uuid),
    PeerDisconnect(Uuid),
    PeerPieces(Uuid, Vec<u8>),
    PeerPiece(Uuid, usize),
    PeerUnchoke(Uuid),
    PeerInterested(Uuid),
    PeerPieceDownloaded(Uuid, Vec<u8>),
    PeerPieceCanceled(Uuid),
    PeerPieceRequest {
        peer_id: Uuid,
        index: u32,
        begin: u32,
        length: u32,
    },
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

async fn download_torrent(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    mut broker_receiver: Receiver<DownloadTorrentEvent>,
) -> Result<(), RustorrentError> {
    let mut torrent_storage = TorrentStorage::new(settings.clone(), torrent_process.clone());

    let (abort_handle, abort_registration) = AbortHandle::new_pair();

    let announce_loop = Abortable::new(
        announce::announce_loop(settings.clone(), torrent_process.clone()).map_err(|e| {
            error!("announce loop error: {}", e);
            e
        }),
        abort_registration,
    )
    .map_err(|e| {
        error!("abortable error: {}", e);
        e.into()
    });

    let mut peer_states = HashMap::new();
    let mut mode = TorrentDownloadMode::Normal;

    let download_torrent_events_loop = async move {
        while let Some(event) = broker_receiver.next().await {
            debug!("received event: {}", event);
            match event {
                DownloadTorrentEvent::Announce(peers) => {
                    debug!("we got announce, what now?");
                    spawn_and_log_error(
                        process_announce(settings.clone(), torrent_process.clone(), peers),
                        || format!("process announce failed"),
                    );
                }
                DownloadTorrentEvent::PeerAnnounced(peer) => {
                    debug!("peer announced: {:?}", peer);
                    if let Err(err) = process_peer_announced(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer.clone(),
                    )
                    .await
                    {
                        error!("cannot process peerannounced {:?}: {}", peer, err);
                    }
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
                    if let Err(err) = process_peer_forwarded(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        stream,
                        &mut torrent_storage,
                    )
                    .await
                    {
                        error!("cannot forward peer: {}", err);
                    }
                }
                DownloadTorrentEvent::PeerConnected(peer_id, stream) => {
                    debug!("[{}] peer connected", peer_id);
                    if let Err(err) = process_peer_connected(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                        stream,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer connected: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerPiece(peer_id, piece) => {
                    debug!("[{}] peer piece: {}", peer_id, piece);
                    if let Err(err) = process_peer_piece(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        &mode,
                        peer_id,
                        piece,
                        &mut torrent_storage,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer piece: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerPieces(peer_id, pieces) => {
                    debug!("[{}] peer pieces", peer_id);
                    if let Err(err) = process_peer_pieces(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        &mode,
                        peer_id,
                        pieces,
                        &mut torrent_storage,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer pieces: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerUnchoke(peer_id) => {
                    debug!("[{}] peer unchoke", peer_id);
                    if let Err(err) = process_peer_unchoke(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer unchoke: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerInterested(peer_id) => {
                    debug!("[{}] peer interested", peer_id);
                    if let Err(err) = process_peer_interested(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer interested: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerPieceCanceled(peer_id) => {
                    debug!("[{}] canceled piece for peer", peer_id);
                    if let Err(err) = process_peer_piece_canceled(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        &mode,
                        peer_id,
                        &mut torrent_storage,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer piece canceled: {}", peer_id, err);
                    }
                }
                DownloadTorrentEvent::PeerPieceDownloaded(peer_id, piece) => {
                    debug!("[{}] downloaded piece for peer", peer_id);
                    if let Err(err) = process_peer_piece_downloaded(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        &mode,
                        peer_id,
                        piece,
                        &mut torrent_storage,
                    )
                    .await
                    {
                        error!(
                            "[{}] cannot process peer piece downloaded: {}",
                            peer_id, err
                        );
                    }

                    mode = determine_download_mode(&mut peer_states, &mut torrent_storage, peer_id);

                    let pieces_left = torrent_storage.receiver.borrow().pieces_left;
                    if pieces_left == 0 {
                        debug!(
                            "torrent downloaded, hash: {}",
                            percent_encode(&torrent_process.hash_id, NON_ALPHANUMERIC)
                        );
                    } else {
                        debug!("pieces left: {}", pieces_left);
                    }
                }
                DownloadTorrentEvent::PeerPieceRequest {
                    peer_id,
                    index,
                    begin,
                    length,
                } => {
                    debug!("[{}] request piece to peer", peer_id);
                    if let Err(err) = process_peer_piece_request(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer_id,
                        index,
                        begin,
                        length,
                        &mut torrent_storage,
                    )
                    .await
                    {
                        error!("[{}] cannot process peer piece request: {}", peer_id, err);
                    }
                }
            }
        }

        abort_handle.abort();

        debug!("download events loop is done");

        Ok::<(), RustorrentError>(())
    };

    let _ = match try_join!(announce_loop, download_torrent_events_loop) {
        Ok(_) | Err(RustorrentError::Aborted) => debug!("download torrent is done"),
        Err(e) => error!("download torrent finished with failure: {}", e),
    };

    debug!("download_torrent done");

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
    let peer_err = peer.clone();
    if let Some((peer_id, existing_peer)) = peer_states_iter.find(|x| x.1.peer == peer) {
        let peer_id = peer_id.clone();
        match existing_peer.state {
            TorrentPeerState::Idle => {
                let handler = spawn_and_log_error(
                    connect_to_peer(settings, torrent_process, peer_id, peer),
                    move || format!("connect to existing peer {} {:?} failed", peer_id, peer_err),
                );
                existing_peer.state = TorrentPeerState::Connecting(handler);
            }
            TorrentPeerState::Connected { .. } => {
                existing_peer.announce_count += 1;
            }
            _ => (),
        }
    } else {
        let peer_id = Uuid::new_v4();
        let torrent_process_on_failure = torrent_process.clone();
        peer_states.insert(
            peer_id,
            PeerState {
                peer: peer.clone(),
                state: TorrentPeerState::Connecting(tokio::spawn(async move {
                    if let Err(err) =
                        connect_to_peer(settings, torrent_process, peer_id, peer).await
                    {
                        error!(
                            "[{}] connect to new peer {:?} failed: {}",
                            peer_id, peer_err, err
                        );
                        if let Err(err) = torrent_process_on_failure
                            .broker_sender
                            .clone()
                            .send(DownloadTorrentEvent::PeerConnectFailed(peer_id))
                            .await
                        {
                            error!("[{}] cannot send peer connect failed: {}", peer_id, err);
                        }
                    }
                })),
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
                downloading_since: None,
                downloaded: 0,
                uploaded: 0,
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

    let _ = spawn_and_log_error(
        peer_loop(settings, torrent_process, peer_id, sender, receiver, stream),
        move || format!("[{}] peer loop failed", peer_id),
    );

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

        let _ = spawn_and_log_error(
            peer_loop(
                settings,
                torrent_process,
                peer_id,
                sender.clone(),
                receiver,
                stream,
            ),
            move || format!("[{}] existing peer loop failed", peer_id),
        );

        existing_peer.state = TorrentPeerState::Connected {
            chocked: true,
            interested: false,
            downloading_piece: None,
            downloading_since: None,
            downloaded: 0,
            uploaded: 0,
            pieces: vec![],
            sender,
        };
    }

    Ok(())
}

fn request_message(buffer: &[u8], piece: usize, piece_length: usize) -> (u32, u32, u32) {
    let index = piece as u32;
    let begin = buffer.len() as u32;
    let length = if piece_length - buffer.len() < BLOCK_SIZE {
        piece_length - buffer.len()
    } else {
        BLOCK_SIZE
    } as u32;
    (index, begin, length)
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
            request: None,
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
                PeerMessage::Piece {
                    index,
                    begin,
                    block,
                } => {
                    debug!(
                        "[{}] sending piece {} {} [{}]",
                        peer_id,
                        index,
                        begin,
                        block.len()
                    );
                    processor
                        .wtransport
                        .send(Message::Piece {
                            index,
                            begin,
                            block,
                        })
                        .await?;
                }
                PeerMessage::Cancel => {
                    debug!("[{}] cancel download", peer_id);
                    if let Some((index, begin, length)) = processor.request {
                        processor
                            .wtransport
                            .send(Message::Cancel {
                                index,
                                begin,
                                length,
                            })
                            .await?;
                        processor.request = None;
                        processor.downloading = None;
                        processor.torrent_piece = None;
                        processor
                            .command_loop_broker_sender
                            .send(DownloadTorrentEvent::PeerPieceCanceled(peer_id))
                            .await?;
                    }
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
                        let (index, begin, length) =
                            request_message(torrent_peer_piece, piece, processor.piece_length);
                        processor.request = Some((index, begin, length));
                        processor
                            .wtransport
                            .send(Message::Request {
                                index,
                                begin,
                                length,
                            })
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
    request: Option<(u32, u32, u32)>,
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

        let peer_id = self.peer_id;
        debug!("[{}] unchocked", peer_id);

        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerUnchoke(self.peer_id))
            .await?;

        debug!(
            "[{}] send DownloadTorrentEvent::PeerUnchoke message",
            peer_id
        );

        debug!(
            "[{}] checking piece progress: {:?}",
            peer_id, self.downloading
        );
        if let Some(piece) = self.downloading {
            if let Some(ref torrent_peer_piece) = self.torrent_piece {
                let (index, begin, length) =
                    request_message(torrent_peer_piece, piece, self.piece_length);
                self.request = Some((index, begin, length));
                self.wtransport
                    .send(Message::Request {
                        index,
                        begin,
                        length,
                    })
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

                use std::cmp::Ordering;
                match self.piece_length.cmp(&torrent_peer_piece.len()) {
                    Ordering::Greater => {
                        let (index, begin, length) =
                            request_message(torrent_peer_piece, piece, self.piece_length);
                        self.request = Some((index, begin, length));
                        self.wtransport
                            .send(Message::Request {
                                index,
                                begin,
                                length,
                            })
                            .await?;
                    }
                    Ordering::Equal => {
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
                    }
                    _ => {
                        error!(
                            "[{}] wrong piece length: {} {}",
                            peer_id,
                            piece,
                            torrent_peer_piece.len()
                        );
                        return Ok(false);
                    }
                }
            }
        } else {
            error!("[{}] abnormal piece message {} for peer", peer_id, index);
        }

        Ok(false)
    }

    async fn request(
        &mut self,
        index: u32,
        begin: u32,
        length: u32,
    ) -> Result<bool, RustorrentError> {
        let peer_id = self.peer_id;

        if !self.interested {
            error!("[{}] peer requested data without unchoke", peer_id);
            return Ok(true);
        }

        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerPieceRequest {
                peer_id,
                index,
                begin,
                length,
            })
            .await?;

        Ok(false)
    }

    async fn interested(&mut self) -> Result<bool, RustorrentError> {
        self.interested = true;
        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerInterested(self.peer_id))
            .await?;

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
            Message::Interested => {
                return self.interested().await;
            }
            Message::Piece {
                index,
                begin,
                block,
            } => {
                return self.piece(index, begin, block).await;
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                return self.request(index, begin, length).await;
            }
            _ => debug!("[{}] unhandled message: {}", peer_id, message),
        }

        Ok(false)
    }
}

/// Peer reveived message Have.
async fn process_peer_piece(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
    peer_piece: usize,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer piece", peer_id);

    let new_pieces = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        match existing_peer.state {
            TorrentPeerState::Connected { .. } => {
                let mut downloadable = vec![];
                let (index, bit) = index_in_bitarray(peer_piece);
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

    select_new_peer(&new_pieces, peer_states, mode, peer_id, storage).await?;

    Ok(())
}

async fn process_peer_pieces(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
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

    select_new_peer(&new_pieces, peer_states, mode, peer_id, storage).await?;

    Ok(())
}

fn determine_download_mode(
    peer_states: &mut HashMap<Uuid, PeerState>,
    storage: &mut TorrentStorage,
    peer_id: Uuid,
) -> TorrentDownloadMode {
    let pieces_left = storage.receiver.borrow().pieces_left;

    let connected_count = peer_states
        .values()
        .filter(|x| match x.state {
            TorrentPeerState::Connected { .. } => true,
            _ => false,
        })
        .count();

    let final_mode = pieces_left < connected_count;

    if final_mode {
        debug!("[{}] select piece in final mode", peer_id);
        TorrentDownloadMode::Final
    } else {
        debug!("[{}] select piece in normal mode", peer_id);
        TorrentDownloadMode::Normal
    }
}

async fn select_new_peer(
    new_pieces: &[usize],
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    for &new_piece in new_pieces {
        if let TorrentDownloadMode::Normal = mode {
            let any_peer_downloading = peer_states.values().any(|x| match x.state {
                TorrentPeerState::Connected {
                    downloading_piece, ..
                } => downloading_piece == Some(new_piece),
                _ => false,
            });
            if any_peer_downloading {
                continue;
            }
        }

        if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
            if let TorrentPeerState::Connected {
                ref mut downloading_piece,
                ref mut downloading_since,
                ref mut sender,
                ..
            } = existing_peer.state
            {
                if downloading_piece.is_none() {
                    *downloading_piece = Some(new_piece);
                    *downloading_since = Some(Instant::now());
                    sender.send(PeerMessage::Download(new_piece)).await?;
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
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
    piece: Vec<u8>,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer piece downloaded", peer_id);

    let (index, new_pieces) = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        if let TorrentPeerState::Connected {
            ref pieces,
            ref mut downloading_piece,
            ref mut downloading_since,
            ..
        } = existing_peer.state
        {
            if let (Some(index), Some(since)) = (downloading_piece.take(), downloading_since.take())
            {
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

    for (peer_id, peer_state) in peer_states.iter_mut().filter(|(&key, _)| key != peer_id) {
        if let TorrentPeerState::Connected {
            ref mut sender,
            ref pieces,
            ref mut downloading_piece,
            ref mut downloading_since,
            ..
        } = peer_state.state
        {
            let peer_already_have_piece = bit_by_index(index, pieces).is_some();
            if peer_already_have_piece {
                continue;
            }
            debug!("[{}] sending Have {}", peer_id, index);
            if let Err(err) = sender.send(PeerMessage::Have(index)).await {
                error!(
                    "[{}] cannot send Have to {:?}: {}",
                    peer_id, peer_state.peer, err
                );
            };

            let peer_downloads_same_piece = *downloading_piece == Some(index);
            if peer_downloads_same_piece {
                if let Err(err) = sender.send(PeerMessage::Cancel).await {
                    error!(
                        "[{}] cannot send Have to {:?}: {}",
                        peer_id, peer_state.peer, err
                    );
                };
            }
        }
    }

    select_new_peer(&new_pieces, peer_states, mode, peer_id, storage).await?;

    Ok(())
}

async fn process_peer_piece_canceled(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    mode: &TorrentDownloadMode,
    peer_id: Uuid,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    debug!("[{}] peer piece downloaded", peer_id);

    let new_pieces = if let Some(existing_peer) = peer_states.get_mut(&peer_id) {
        if let TorrentPeerState::Connected {
            ref pieces,
            ref mut downloading_piece,
            ref mut downloading_since,
            ..
        } = existing_peer.state
        {
            *downloading_piece = None;
            *downloading_since = None;
            let mut downloadable = vec![];
            for (i, &a) in pieces.iter().enumerate() {
                match_pieces(
                    &mut downloadable,
                    &storage.receiver.borrow().downloaded,
                    i,
                    a,
                );
            }
            downloadable
        } else {
            return Ok(());
        }
    } else {
        return Ok(());
    };

    select_new_peer(&new_pieces, peer_states, mode, peer_id, storage).await?;

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

/// Adds matching (new) pieces ( downloaded_pieces[i] & a ) to pieces (list of indexes).
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

async fn process_peer_interested(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
) -> Result<(), RustorrentError> {
    debug!("[{}] process peer interested", peer_id);

    if let Some(TorrentPeerState::Connected {
        ref mut interested, ..
    }) = peer_states.get_mut(&peer_id).map(|x| &mut x.state)
    {
        *interested = true;
    }

    Ok(())
}

async fn process_peer_piece_request(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut HashMap<Uuid, PeerState>,
    peer_id: Uuid,
    index: u32,
    begin: u32,
    length: u32,
    storage: &mut TorrentStorage,
) -> Result<(), RustorrentError> {
    if let Some(TorrentPeerState::Connected { ref mut sender, .. }) =
        peer_states.get_mut(&peer_id).map(|x| &mut x.state)
    {
        if let Some(piece) = storage.load(index as usize).await? {
            let block = piece.as_ref()[begin as usize..(begin as usize + length as usize)].to_vec();
            sender
                .send(PeerMessage::Piece {
                    index,
                    begin,
                    block,
                })
                .await?;
        }
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

        let result = collect_pieces_and_update(&mut current_pieces, &[0b1010_1010], &[0b010_10101]);
        assert_eq!(result, vec![0, 2, 4, 6]);
        assert_eq!(current_pieces, vec![0b1010_1010]);

        let mut current_pieces = vec![];

        let result = collect_pieces_and_update(&mut current_pieces, &[0b1010_1010], &[0b1101_0101]);
        assert_eq!(result, vec![2, 4, 6]);
        assert_eq!(current_pieces, vec![0b1010_1010]);
    }

    fn test_settings() -> Arc<Settings> {
        Arc::new(Default::default())
    }

    #[tokio::test]
    async fn check_process_peer_pieces() {}
}
