use std::ops::Deref;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use {
    crate::{errors::RustorrentError, types::torrent::parse_torrent, PEER_ID},
    std::{
        collections::HashMap,
        convert::TryInto,
        mem::{self, drop},
        net::{IpAddr, Ipv4Addr, SocketAddr},
        path::{Path, PathBuf},
    },
};

use exitfailure::ExitFailure;
use failure::{Context, ResultExt};
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::future::try_join;
use futures::future::{join_all, lazy};
use futures::prelude::*;
use log::{debug, error, info, warn};
// use tokio::codec::Decoder;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio::time::{delay_for, Interval};

use crate::types::info::TorrentInfo;
use crate::types::message::{Message, MessageCodec, MessageCodecError};
use crate::types::peer::Handshake;
use crate::types::peer::Peer;
use crate::types::torrent::{Torrent, TrackerAnnounce};
use crate::types::Settings;
use crate::{count_parts, SHA1_SIZE};

pub struct RustorrentApp {
    pub settings: Settings,
}

#[derive(Debug)]
pub struct TorrentProcess {
    pub(crate) path: PathBuf,
    pub(crate) torrent: Torrent,
    pub(crate) info: TorrentInfo,
    pub(crate) hash_id: [u8; SHA1_SIZE],
    pub(crate) torrent_state: Arc<Mutex<TorrentProcessState>>,
    pub(crate) announce_state: Arc<Mutex<AnnounceState>>,
    pub(crate) blocks_downloading: Arc<Mutex<HashMap<Block, Arc<TorrentPeer>>>>,
    pub(crate) stats: Arc<Mutex<TorrentProcessStats>>,
    pub(crate) torrent_storage: RwLock<TorrentStorage>,
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
        Self {
            settings,
        }
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
        let ipv4 = config.ipv4.or_else(|| Some(Ipv4Addr::new(0, 0, 0, 0)));
        let addr = match (ipv4, config.port) {
            (Some(ipv4), Some(port)) => SocketAddr::new(ipv4.into(), port),
            _ => return Err(RustorrentError::WrongConfig),
        };

        let (broker_sender, broker_receiver) = mpsc::unbounded();

        let _broker_handle = tokio::spawn(async {
            broker_loop(broker_receiver).await;
        });


        let accept_incoming_connections = async move {
            eprintln!("listening on: {}", &addr);
            let mut listener = TcpListener::bind(addr).await?;
            
            loop {
                let (mut socket, _) = listener.accept().await?;
                let _ = tokio::spawn(async {
                    let mut buf = [0; 1024];
                    eprintln!("yahoo");
                });
            }
            Ok::<(), RustorrentError>(())
        };
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

async fn broker_loop(mut events: mpsc::UnboundedReceiver<RustorrentEvent>) -> Result<(), RustorrentError> {
    Ok(())
}
