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
use futures::try_ready;
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
use crate::messages::message_bitfield;
use crate::types::info::TorrentInfo;
use crate::types::message::{Message, MessageCodec, MessageCodecError};
use crate::types::peer::Handshake;
use crate::types::peer::Peer;
use crate::types::torrent::parse_torrent;
use crate::types::torrent::{Torrent, TrackerAnnounce};
use crate::types::Settings;
use crate::SHA1_SIZE;

const TWO_MINUTES: Duration = Duration::from_secs(120);

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
pub enum AnnounceState {
    Idle,
    Request,
    Error(Arc<RustorrentError>),
}

enum RustorrentCommand {
    PeerMessage(Arc<TorrentProcess>, Arc<TorrentPeer>, Message),
    ConnectToPeer(Arc<TorrentProcess>, Arc<TorrentPeer>),
    ProcessAnnounce(Arc<TorrentProcess>, TrackerAnnounce),
    ProcessAnnounceError(Arc<TorrentProcess>, Arc<RustorrentError>),
    AddTorrent(PathBuf),
    Quit,
}

const PEER_ID: [u8; 20] = *b"-rs0001-zzzzxxxxyyyy";

fn url_encode(data: &[u8]) -> String {
    data.iter()
        .map(|&x| percent_encode_byte(x))
        .collect::<String>()
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

    pub fn command_quit(self: Arc<Self>) -> Result<(), RustorrentError> {
        self.send_command(RustorrentCommand::Quit)
    }

    fn send_command(self: Arc<Self>, command: RustorrentCommand) -> Result<(), RustorrentError> {
        tokio::spawn(
            self.command_sender
                .clone()
                .send(command)
                .map(|_| ())
                .map_err(|err| error!("send failed: {}", err)),
        );
        Ok(())
    }

    fn command_add_torrent(
        self: Arc<Self>,
        path: PathBuf,
    ) -> Result<Arc<TorrentProcess>, RustorrentError> {
        debug!("Run command: adding torrent from file: {:?}", path);
        let torrent = parse_torrent(&path)?;
        let hash_id = torrent.info_sha1_hash();
        if let Some(process) = self
            .processes
            .read()
            .unwrap()
            .iter()
            .filter(|x| x.hash_id == hash_id)
            .cloned()
            .next()
        {
            warn!("Torrent already in the list: {}", url_encode(&hash_id));
            return Ok(process);
        }
        let info = torrent.info()?;
        let left = info.len();
        let pieces_count = info.pieces.len();
        let process = Arc::new(TorrentProcess {
            path,
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
            torrent_storage: RwLock::new(TorrentStorage {
                pieces: vec![Arc::new(Mutex::new(Default::default())); pieces_count],
                peers: vec![],
            }),
        });
        self.processes.write().unwrap().push(process.clone());
        Ok(process)
    }

    fn command_start_announce_process(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
    ) -> Result<(), RustorrentError> {
        {
            let mut announce_state = torrent_process.announce_state.lock().unwrap();
            match *announce_state {
                AnnounceState::Idle => {
                    *announce_state = AnnounceState::Request;
                }
                _ => {
                    debug!("torrent process announce already running");
                    return Ok(());
                }
            }
        }

        let client = Client::new();

        let mut url = {
            let stats = torrent_process.stats.lock().unwrap();
            format!(
                "{}?info_hash={}&peer_id={}&left={}",
                torrent_process.torrent.announce_url,
                url_encode(&torrent_process.hash_id[..]),
                url_encode(&PEER_ID[..]),
                stats.left
            )
        };

        let config = &self.settings.config;

        if let Some(port) = config.port {
            url += format!("&port={}", port).as_str();
        }

        if let Some(compact) = config.compact {
            url += format!("&compact={}", if compact { 1 } else { 0 }).as_str();
        }

        debug!("Get tracker announce from: {}", url);

        let announce_state_succ = torrent_process.announce_state.clone();
        let announce_state_err = torrent_process.announce_state.clone();

        let this_response = self.clone();
        let this_err = self.clone();
        let torrent_process_response = torrent_process.clone();
        let torrent_process_err = torrent_process.clone();

        let process = client
            .get(&url)
            .send()
            .and_then(|mut res| {
                debug!("Result code: {}", res.status());

                let body = mem::replace(res.body_mut(), ReqwestDecoder::empty());
                body.concat2()
            })
            .and_then(|body| {
                let mut buf = vec![];
                let mut body = std::io::Cursor::new(body);
                std::io::copy(&mut body, &mut buf).unwrap();
                Ok(buf)
            })
            .map_err(RustorrentError::from)
            .and_then(move |response| {
                debug!(
                    "Tracker response (url encoded): {}",
                    percent_encode(&response, SIMPLE_ENCODE_SET).to_string()
                );
                let tracker_announce: TrackerAnnounce = response.try_into()?;
                debug!("Tracker response parsed: {:#?}", tracker_announce);
                *announce_state_succ.lock().unwrap() = AnnounceState::Idle;
                let process_announce =
                    RustorrentCommand::ProcessAnnounce(torrent_process_response, tracker_announce);
                this_response.send_command(process_announce)?;
                Ok(())
            })
            .map_err(move |err| {
                error!("Error in announce request: {}", err);
                let err = Arc::new(err);
                *announce_state_err.lock().unwrap() = AnnounceState::Error(err.clone());
                let process_announce =
                    RustorrentCommand::ProcessAnnounceError(torrent_process_err, err);
                this_err
                    .send_command(process_announce)
                    .map_err(|err| error!("{}", err))
                    .unwrap();
            });
        tokio::spawn(process);
        Ok(())
    }

    fn spawn_delayed_announce(
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
            .map_err(|_| ());
        tokio::spawn(task);
        Ok(())
    }

    fn command_peer_message(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
        message: Message,
    ) -> Result<(), RustorrentError> {
        info!("Handle message: {:?}", message);

        match message {
            Message::Bitfield(bitfield_pieces) => {
                message_bitfield(torrent_process, torrent_peer, bitfield_pieces)?;
            }
            _ => warn!("Unsupported message {:?}", message),
        }

        Ok(())
    }

    fn command_connect_to_peer(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        torrent_peer: Arc<TorrentPeer>,
    ) -> Result<(), RustorrentError> {
        let (tx, rx) = channel(10);

        *torrent_peer.state.lock().unwrap() = TorrentPeerState::Connecting;

        let conntx = tx.clone();
        let addr = torrent_peer.addr;
        let task_keepalive = Interval::new(Instant::now(), TWO_MINUTES)
            .for_each(move |_| {
                debug!("Peer {}: sending message KeepAlive", addr);
                let conntx = conntx.clone();
                conntx
                    .send(Message::KeepAlive)
                    .map(|_| ())
                    .map_err(|_| tokio::timer::Error::shutdown())
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

                let writer = writer.sink_map_err(|err| error!("{}", err));

                let sink = rx.forward(writer).inspect(move |(_a, _sink)| {
                    debug!("Peer {}: updated", addr);
                });
                tokio::spawn(sink.map(|_| ()));

                *torrent_peer_handshake_done.state.lock().unwrap() = TorrentPeerState::Connected {
                    chocked: true,
                    interested: false,
                    sender: conntx_state.clone(),
                    pieces: vec![],
                };

                let conn = reader
                    .for_each(move |frame| {
                        debug!("Peer {}: received message {:?}", addr, frame);
                        match frame {
                            Message::KeepAlive => {
                                let conntx = tx.clone();
                                tokio::spawn(
                                    conntx.send(Message::KeepAlive).map(|_| ()).map_err(|_| ()),
                                );
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

    fn command_process_announce(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
        tracker_announce: TrackerAnnounce,
    ) -> Result<(), RustorrentError> {
        info!("time to process announce");
        match *torrent_process.announce_state.lock().unwrap() {
            AnnounceState::Idle => {
                self.clone().spawn_delayed_announce(
                    torrent_process.clone(),
                    Duration::from_secs(tracker_announce.interval as u64),
                )?;
            }
            AnnounceState::Error(ref error) => {
                return Err(RustorrentError::FailureReason(format!(
                    "Announce failure: {}",
                    error
                )))
            }
            ref state => {
                return Err(RustorrentError::FailureReason(format!(
                    "Wrong state: {:?}",
                    state
                )))
            }
        }

        let mut torrent_storage = torrent_process.torrent_storage.write().unwrap();
        for peer in &tracker_announce.peers {
            let addr = SocketAddr::new(IpAddr::V4(peer.ip), peer.port);
            if let Some(existing_peer) = torrent_storage
                .peers
                .iter()
                .filter(|x| x.addr == addr)
                .next()
            {
                info!("Checking peer: {:?}", peer);
                let announcement_count = existing_peer
                    .announcement_count
                    .fetch_add(1, Ordering::SeqCst);
                debug!(
                    "Peer {:?} announced {} time(s)",
                    peer,
                    announcement_count + 1
                );
                match *existing_peer.state.lock().unwrap() {
                    TorrentPeerState::Idle => {
                        info!("Reconnecting to peer: {:?}", peer);

                        let connect_to_peer = RustorrentCommand::ConnectToPeer(
                            torrent_process.clone(),
                            existing_peer.clone(),
                        );
                        self.clone().send_command(connect_to_peer)?;
                    }
                    _ => (),
                }
            } else {
                info!("Adding peer: {:?}", peer);
                let peer: Arc<TorrentPeer> = Arc::new(peer.into());
                torrent_storage.peers.push(peer.clone());
                let connect_to_peer =
                    RustorrentCommand::ConnectToPeer(torrent_process.clone(), peer);
                self.clone().send_command(connect_to_peer)?;
            }
        }

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
            .map_err(|_| ());
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
                }
                Ok(())
            })
            .select2(close_receiver)
            .map_err(|_| ())
            .then(|_| Ok(()))
    }
}
