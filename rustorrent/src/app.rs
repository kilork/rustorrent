use crate::PEER_ID;
use std::collections::HashMap;
use std::convert::TryInto;
use std::mem;
use std::mem::drop;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use exitfailure::ExitFailure;
use failure::{Context, ResultExt};
use futures::future::join_all;
use futures::lazy;
use futures::prelude::*;
use futures::sync::mpsc::{channel, Receiver, Sender};
use log::{debug, error, info, warn};
use percent_encoding::{percent_encode, percent_encode_byte, SIMPLE_ENCODE_SET};
use reqwest::r#async::{Client, Decoder as ReqwestDecoder};
use tokio::codec::Decoder;
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::timer::{Delay, Interval};

use crate::errors::RustorrentError;
use crate::types::info::TorrentInfo;
use crate::types::message::{Message, MessageCodec, MessageCodecError};
use crate::types::peer::Handshake;
use crate::types::peer::Peer;
use crate::types::torrent::{Torrent, TrackerAnnounce};
use crate::types::Settings;
use crate::SHA1_SIZE;

pub struct RustorrentApp {
    inner: Arc<Inner>,
}

impl Deref for RustorrentApp {
    type Target = Arc<Inner>;
    fn deref(&self) -> &Arc<Inner> {
        &self.inner
    }
}

pub struct Inner {
    pub settings: Settings,
    pub processes: RwLock<Vec<Arc<TorrentProcess>>>,
    command_sender: UnboundedSender<RustorrentCommand>,
    command_receiver: Mutex<Option<UnboundedReceiver<RustorrentCommand>>>,
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
}

#[derive(Debug)]
pub(crate) struct TorrentPeer {
    pub(crate) addr: SocketAddr,
    pub(crate) announcement_count: AtomicUsize,
    pub(crate) state: Mutex<TorrentPeerState>,
}

impl From<&Peer> for TorrentPeer {
    fn from(value: &Peer) -> Self {
        let addr = SocketAddr::new(IpAddr::V4(value.ip), value.port);
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
        sender: Sender<Message>,
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

#[derive(Debug)]
pub enum TorrentProcessState {
    Init,
    Download,
    DownloadUpload,
    Upload,
    Checksum,
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
    AddTorrent(PathBuf),
    Quit,
}

impl Inner {
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
        let task = Delay::new(when)
            .map_err(RustorrentError::from)
            .and_then(|_| {
                info!("time to reannounce!");
                self.command_start_announce_process(torrent_process)?;
                Ok(())
            })
            .map_err(|err| error!("Delayed task failed: {}", err));

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
}

impl RustorrentApp {
    pub fn new(settings: Settings) -> Self {
        let (command_sender, command_receiver) = unbounded_channel();
        Self {
            inner: Arc::new(Inner {
                settings,
                processes: RwLock::new(vec![]),
                command_sender,
                command_receiver: Mutex::new(Some(command_receiver)),
            }),
        }
    }

    pub fn run(&mut self) -> impl Future<Item = (), Error = RustorrentError> {
        let is_running = Arc::new(AtomicBool::new(true));

        let can_try_count = Arc::new(AtomicUsize::new(10));

        self.clone().start_info_update_loop(is_running.clone());

        let receiver = self.command_receiver.lock().unwrap().take().unwrap();
        let (close_sender, close_receiver) = futures::sync::oneshot::channel::<()>();
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
                            if err.is_http() {
                                if let Some(err) =
                                    err.get_ref().and_then(|e| e.downcast_ref::<hyper::Error>())
                                {
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
                }

                Ok(())
            })
            .select2(close_receiver)
            .map_err(|_err| error!("Error in run loop"))
            .then(|_| Ok(()))
    }
}
