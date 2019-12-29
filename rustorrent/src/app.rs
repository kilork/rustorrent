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
    // pub(crate) torrent_state: Arc<Mutex<TorrentProcessState>>,
    // pub(crate) announce_state: Arc<Mutex<AnnounceState>>,
    // pub(crate) blocks_downloading: Arc<Mutex<HashMap<Block, Arc<TorrentPeer>>>>,
    // pub(crate) stats: Arc<Mutex<TorrentProcessStats>>,
    // pub(crate) torrent_storage: RwLock<TorrentStorage>,
}

#[derive(Debug)]
pub(crate) struct TorrentStorage {
    pub(crate) pieces: Vec<Arc<Mutex<TorrentPiece>>>,
    pub(crate) peers: Vec<Arc<TorrentPeer>>,
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
pub(crate) struct TorrentPeer {
    pub(crate) addr: SocketAddr,
    pub(crate) announcement_count: AtomicUsize,
    pub(crate) state: Mutex<TorrentPeerState>,
}

impl From<&Peer> for TorrentPeer {
    fn from(value: &Peer) -> Self {
        let addr = SocketAddr::new(value.ip, value.port);
        Self {
            addr,
            announcement_count: AtomicUsize::new(0),
            state: Mutex::new(Default::default()),
        }
    }
}

#[derive(Debug)]
pub(crate) enum TorrentPeerState {
    Idle,
    Connecting,
    Connected {
        chocked: bool,
        interested: bool,
        downloading: bool,
        sender: UnboundedSender<Message>,
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

pub(crate) enum RustorrentCommand {
    PeerMessage(Arc<TorrentProcess>, Arc<TorrentPeer>, Message),
    ConnectToPeer(Arc<TorrentProcess>, Arc<TorrentPeer>),
    DownloadBlock(Arc<TorrentProcess>, Arc<TorrentPeer>, Block),
    ProcessAnnounce(Arc<TorrentProcess>, TrackerAnnounce),
    ProcessAnnounceError(Arc<TorrentProcess>, Arc<RustorrentError>),
    PieceDownloaded(Arc<TorrentProcess>, Arc<TorrentPeer>, usize),
    DownloadNextBlock(Arc<TorrentProcess>, Arc<TorrentPeer>),
    AddTorrent(PathBuf),
    Quit,
}

pub(crate) enum RustorrentEvent {
    AddTorrent(PathBuf),
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

        let (mut download_events_sender, download_events_receiver) = mpsc::unbounded();

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

async fn accept_connections_loop(
    settings: Arc<Settings>,
    addr: SocketAddr,
    mut sender: UnboundedSender<RustorrentEvent>,
) -> Result<(), RustorrentError> {
    eprintln!("listening on: {}", &addr);
    let mut listener = TcpListener::bind(addr).await?;

    loop {
        let (mut socket, _) = listener.accept().await?;
        let _ = tokio::spawn(async {
            let mut buf = [0; 1024];
            eprintln!("yahoo");
        });
    }

    Ok(())
}

async fn download_events_loop(
    settings: Arc<Settings>,
    mut sender: UnboundedSender<RustorrentEvent>,
    mut events: UnboundedReceiver<RustorrentEvent>,
) -> Result<(), RustorrentError> {
    let mut torrents = vec![];

    while let Some(event) = events.next().await {
        match event {
            RustorrentEvent::AddTorrent(filename) => {
                eprintln!("we need to download {:?}", filename);
                let torrent = parse_torrent(&filename)?;
                let hash_id = torrent.info_sha1_hash();
                let info = torrent.info()?;
                let torrent_process = Arc::new(TorrentProcess {
                    info,
                    hash_id,
                    torrent,
                });
                let _ = tokio::spawn(download_torrent(settings.clone(), torrent_process.clone()));
                torrents.push(torrent_process);
            }
        }
    }

    Ok(())
}

enum DownloadTorrentEvent {
    Announce(TrackerAnnounce),
}

async fn download_torrent(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
) -> Result<(), RustorrentError> {
    let (mut broker_sender, mut broker_receiver) = mpsc::unbounded();

    let _ = tokio::spawn(async move {
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

                    broker_sender
                        .send(DownloadTorrentEvent::Announce(tracker_announce))
                        .await?;
                    Duration::from_secs(interval_to_reannounce)
                }

                Err(err) => {
                    error!("Failure {}, retry in 5 seconds", err);
                    Duration::from_secs(5)
                }
            };

            delay_for(interval_to_query_tracker).await;
        }

        Ok::<(), RustorrentError>(())
    });

    while let Some(event) = broker_receiver.next().await {
        debug!("received event");
        match event {
            DownloadTorrentEvent::Announce(tracker_announce) => {
                debug!("we got announce, that now?");
            }
        }
    }

    Ok(())
}

/*
enum ProcessPeerEvent {}

async fn process_peer(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
    download_torrent_broker_sender: UnboundedSender<DownloadTorrentEvent>, // peer:
) -> Result<(), RustorrentError> {
    let (mut broker_sender, mut broker_receiver) = mpsc::unbounded();

    let _ = tokio::spawn(async move {});

    while let Some(event) = broker_receiver.next().await {
        debug!("received event");
    }

    Ok(())
}

*/
