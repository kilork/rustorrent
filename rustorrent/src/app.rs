use super::*;
use crate::{errors::RustorrentError, types::torrent::parse_torrent, PEER_ID};

// use tokio::codec::Decoder;

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
pub(crate) struct TorrentStorage {
    pub(crate) pieces: Vec<Arc<Mutex<TorrentPiece>>>,
}

#[derive(Debug, Default)]
pub(crate) struct TorrentPiece {
    pub(crate) downloaded: bool,
    pub(crate) data: Vec<u8>,
    pub(crate) blocks: Vec<u8>,
    pub(crate) blocks_to_download: usize,
}

impl TorrentPiece {
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
    Finished,
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

#[derive(Debug)]
pub(crate) enum AnnounceState {
    Idle,
    Request,
    Error(Arc<RustorrentError>),
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
}

/*
pub fn add_torrent_from_file(
    self: Arc<Self>,
    filename: impl AsRef<Path>,
) -> Result<(), RustorrentError> {
    info!("Adding torrent from file: {:?}", filename.as_ref());
    let command = RustorrentCommand::AddTorrent(filename.as_ref().into());
    self.send_command(command)
}

pub(crate) fn command_quit(self: Arc<Self>) -> Result<(), RustorrentError> {
    self.send_command(RustorrentCommand::Quit)
}

pub(crate) fn send_command(
    self: Arc<Self>,
    command: RustorrentCommand,
) -> Result<(), RustorrentError> {
    tokio::spawn(
        self.command_sender
            .clone()
            .send(command)
            .map(|_| ())
            .map_err(|err| error!("send failed: {}", err)),
    );

    Ok(())
}

pub(crate) fn spawn_delayed_announce(
    self: Arc<Self>,
    torrent_process: Arc<TorrentProcess>,
    after: Duration,
) -> Result<(), RustorrentError> {
    let when = Instant::now() + after;
    let task = delay(when)
        /*.and_then(|_| {
            info!("time to reannounce!");
            self.command_start_announce_process(torrent_process)?;
            Ok(())
        })
        .map_err(|err| error!("Delayed task failed: {}", err));*/

    tokio::spawn(task);

    Ok(())
}

fn start_info_update_loop(self: Arc<Self>, is_running: Arc<AtomicBool>) {
    let interval = Interval::new(Instant::now(), Duration::from_secs(10));

    let is_running_clone = is_running.clone();
    let interval_task = interval
        .map_err(RustorrentError::from)
        .take_while(move |_| Ok(is_running_clone.load(Ordering::SeqCst)))
        .for_each(move |_| {
            for process in self.processes.read().unwrap().iter() {
                let announce_state = process.announce_state.lock().unwrap();
                let torrent_state = process.torrent_state.lock().unwrap();
                let stats = process.stats.lock().unwrap();
                info!(
                    "{:?} {:?} {:?} {:?}",
                    process.path, announce_state, torrent_state, stats
                );
            }
            Ok(())
        })
        .map_err(|err| error!("Info update loop failure: {}", err));
    tokio::spawn(interval_task);
}
*/

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

    /*
        pub async fn run(&mut self) -> Result<(), RustorrentError> {
            let is_running = Arc::new(AtomicBool::new(true));

            let can_try_count = Arc::new(AtomicUsize::new(10));

            self.clone().start_info_update_loop(is_running.clone());

            let receiver = self.command_receiver.lock().unwrap().take().unwrap();
            let (close_sender, close_receiver) = futures::channel::oneshot::channel::<()>();
            let close_sender = Arc::new(Mutex::new(Some(close_sender)));
            let this = self.clone();
            receiver
                .map_err(RustorrentError::from)
                .for_each(move |x| {
                    let this = this.clone();
                    let can_try_count = can_try_count.clone();
                    match x {
                        RustorrentCommand::AddTorrent(filename) => {
                            let this_announce = this.clone();
                            this.command_add_torrent(filename)
                                .and_then(|torrent_process| {
                                    this_announce.command_start_announce_process(torrent_process)
                                })?;
                        }
                        RustorrentCommand::ProcessAnnounceError(torrent_process, err) => match *err {
                            RustorrentError::HTTPClient(ref err) => {
                                if err.is_connect() {
                                    error!("connection refused!");
                                    if can_try_count.fetch_sub(1, Ordering::SeqCst) == 0 {
                                        error!("Cannot connect to announce server, giving up");
                                        this.clone().command_quit()?;
                                    }

                                    *torrent_process.announce_state.lock().unwrap() =
                                        AnnounceState::Idle;
                                    this.spawn_delayed_announce(
                                        torrent_process,
                                        Duration::from_secs(5),
                                    )?;
                                }
                            }
                            ref other => error!("Process announce error: {}", other),
                        },
                        RustorrentCommand::Quit => {
                            info!("Quit now");
                            let sender = close_sender.lock().unwrap().take().unwrap();
                            sender.send(()).unwrap();
                            is_running.store(false, Ordering::SeqCst);
                        }
                        RustorrentCommand::ProcessAnnounce(torrent_process, tracker_announce) => {
                            this.command_process_announce(torrent_process, tracker_announce)?;
                        }
                        RustorrentCommand::ConnectToPeer(torrent_process, torrent_peer) => {
                            this.command_connect_to_peer(torrent_process, torrent_peer)?;
                        }
                        RustorrentCommand::PeerMessage(torrent_process, torrent_peer, message) => {
                            this.command_peer_message(torrent_process, torrent_peer, message)?;
                        }
                        RustorrentCommand::DownloadBlock(torrent_process, torrent_peer, block) => {
                            this.command_download_block(torrent_process, torrent_peer, block)?;
                        }
                        RustorrentCommand::PieceDownloaded(torrent_process, torrent_peer, piece) => {
                            this.command_piece_downloaded(torrent_process, torrent_peer, piece)?;
                        }
                        RustorrentCommand::DownloadNextBlock(torrent_process, torrent_peer) => {
                            this.command_download_next_block(torrent_process, torrent_peer)?;
                        }
                    }

                    Ok(())
                })
                .select2(close_receiver)
                .map_err(|_err| error!("Error in run loop"))
                .then(|_| Ok(()))
        }
    */
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
    PeerConnected(Peer, TcpStream),
    PeerConnectFailed(Peer),
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
    let torrent_data: Vec<u8> = vec![0; torrent_process.info.len()];

    let (abort_handle, abort_registration) = AbortHandle::new_pair();

    let announce_loop = Abortable::new(
        announce_loop(settings.clone(), torrent_process.clone()),
        abort_registration,
    )
    .map_err(|e| e.into());

    let mut peer_states = vec![];

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
                DownloadTorrentEvent::PeerConnectFailed(peer) => {
                    if let Some(index) = peer_states.iter().position(|x| x.peer == peer) {
                        peer_states.remove(index);
                    }
                }
                DownloadTorrentEvent::PeerConnected(peer, stream) => {
                    debug!("peer connected: {:?}", peer);
                    process_peer_connected(
                        settings.clone(),
                        torrent_process.clone(),
                        &mut peer_states,
                        peer,
                        stream,
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
    peer_states: &mut Vec<PeerState>,
    peer: Peer,
) -> Result<(), RustorrentError> {
    let mut peer_states_iter = peer_states.iter_mut();
    if let Some(existing_peer) = peer_states_iter.find(|x| x.peer == peer) {
        match existing_peer.state {
            TorrentPeerState::Idle => {
                let handler = spawn_and_log_error(connect_to_peer(settings, torrent_process, peer));
                existing_peer.state = TorrentPeerState::Connecting(handler);
            }
            TorrentPeerState::Connected { .. } => {
                existing_peer.announce_count += 1;
            }
            _ => (),
        }
    } else {
        peer_states.push(PeerState {
            peer: peer.clone(),
            state: TorrentPeerState::Connecting(spawn_and_log_error(connect_to_peer(
                settings,
                torrent_process,
                peer,
            ))),
            announce_count: 0,
        })
    };

    Ok(())
}

async fn connect_to_peer(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
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
            .send(DownloadTorrentEvent::PeerConnectFailed(peer))
            .await?;
        return Ok(());
    }

    torrent_process
        .broker_sender
        .clone()
        .send(DownloadTorrentEvent::PeerConnected(peer, stream))
        .await?;

    Ok(())
}

async fn process_peer_connected(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    peer_states: &mut Vec<PeerState>,
    peer: Peer,
    stream: TcpStream,
) -> Result<(), RustorrentError> {
    debug!("peer connection initiated: {:?}", peer);

    if let Some(existing_peer) = peer_states.iter_mut().find(|x| x.peer == peer) {
        let (sender, receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let _ = spawn_and_log_error(peer_loop(
            settings,
            torrent_process,
            peer,
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
    peer: Peer,
    mut sender: Sender<PeerMessage>,
    mut receiver: Receiver<PeerMessage>,
    stream: TcpStream,
) -> Result<(), RustorrentError> {
    let (mut wtransport, mut rtransport) = Framed::new(stream, MessageCodec).split();

    let command_loop = async move {
        while let Some(message) = receiver.next().await {
            debug!("peer loop received message: {:?}", message);
            match message {
                PeerMessage::Disconnect => break,
                PeerMessage::Message(message) => (),
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

        sender.send(PeerMessage::Disconnect).await?;

        Ok::<(), RustorrentError>(())
    };

    let _ = try_join!(command_loop, receive_loop)?;

    debug!("peer loop exit");

    Ok(())
}
