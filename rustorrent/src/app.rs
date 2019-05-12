use crate::types::peer::Handshake;
use std::cell::RefCell;
use std::convert::TryInto;
use std::mem;
use std::mem::drop;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;
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
// use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
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
use crate::types::message::{Message, MessageCodec, MessageCodecError};
use crate::types::peer::Peer;
use crate::types::torrent::parse_torrent;
use crate::types::torrent::{Torrent, TrackerAnnounce};
use crate::types::Settings;

pub struct RustorrentApp {
    inner: Arc<Inner>,
}

impl Deref for RustorrentApp {
    type Target = Arc<Inner>;
    fn deref(&self) -> &Arc<Inner> {
        return &self.inner;
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
    path: PathBuf,
    torrent: Torrent,
    hash_id: [u8; 20],
    torrent_state: Arc<Mutex<TorrentProcessState>>,
    announce_state: Arc<Mutex<AnnounceState>>,
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
    Error(RustorrentError),
}

pub enum RustorrentCommand {
    ProcessAnnounce(Arc<TorrentProcess>, TrackerAnnounce),
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
        let process = Arc::new(TorrentProcess {
            path,
            torrent,
            hash_id,
            torrent_state: Arc::new(Mutex::new(TorrentProcessState::Init)),
            announce_state: Arc::new(Mutex::new(AnnounceState::Idle)),
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

        let mut url = format!(
            "{}?info_hash={}&peer_id={}",
            torrent_process.torrent.announce_url,
            url_encode(&torrent_process.hash_id[..]),
            url_encode(&PEER_ID[..])
        );

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

        let this = self.clone();

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
            .map_err(|err| RustorrentError::from(err))
            .and_then(move |response| {
                debug!(
                    "Tracker response (url encoded): {}",
                    percent_encode(&response, SIMPLE_ENCODE_SET).to_string()
                );
                let tracker_announce: TrackerAnnounce = response.try_into()?;
                debug!("Tracker response parsed: {:#?}", tracker_announce);
                *announce_state_succ.lock().unwrap() = AnnounceState::Idle;
                let process_announce =
                    RustorrentCommand::ProcessAnnounce(torrent_process.clone(), tracker_announce);
                this.send_command(process_announce)?;
                Ok(())
            })
            .map_err(move |err| {
                error!("Error in announce request: {}", err);
                *announce_state_err.lock().unwrap() = AnnounceState::Error(err);
            });
        tokio::spawn(process);
        Ok(())
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
        let receiver = self.command_receiver.lock().unwrap().take().unwrap();
        let (close_sender, close_receiver) = futures::sync::oneshot::channel::<()>();
        let close_sender = Arc::new(Mutex::new(Some(close_sender)));
        let this = self.clone();
        receiver
            .map_err(|err| RustorrentError::from(err))
            .for_each(move |x| {
                let this = this.clone();
                let this_announce = this.clone();
                match x {
                    RustorrentCommand::AddTorrent(filename) => {
                        this.command_add_torrent(filename)
                            .and_then(|torrent_process| {
                                this_announce.command_start_announce_process(torrent_process)
                            })?;
                    }
                    RustorrentCommand::Quit => {
                        info!("Quit now");
                        let sender = close_sender.lock().unwrap().take().unwrap();
                        sender.send(()).unwrap();
                    }
                    RustorrentCommand::ProcessAnnounce(process, tracker_announce) => {
                        info!("time to process announce");
                        let state = process.announce_state.lock().unwrap();
                        match *state {
                            AnnounceState::Idle => {
                                let process_copy_delay = process.clone();
                                let when = Instant::now()
                                    + Duration::from_secs(tracker_announce.interval as u64);
                                let task = Delay::new(when)
                                    .map_err(|err| RustorrentError::from(err))
                                    .and_then(|_| {
                                        info!("time to reannounce!");
                                        this.command_start_announce_process(process_copy_delay)?;
                                        Ok(())
                                    })
                                    .map_err(|_| ());
                                tokio::spawn(task);
                            }
                            _ => return Err(RustorrentError::FailureReason("Qqq".into())),
                        }
                    }
                }
                Ok(())
            })
            .select2(close_receiver)
            .map_err(|_| ())
            .then(|_| Ok(()))
    }
}

/*
pub struct TorrentProcessFeature {
    pub process: Arc<TorrentProcess>,
    pub state: TorrentProcessState,
    pub settings: Arc<Settings>,
}

impl TorrentProcessFeature {
    fn announce_request(&mut self) -> Poll<(), ()> {
        let client = Client::new();

        let mut url = format!(
            "{}?info_hash={}&peer_id={}",
            self.process.torrent.announce_url,
            url_encode(&self.process.hash_id[..]),
            url_encode(&PEER_ID[..])
        );

        let config = &self.settings.config;

        if let Some(port) = config.port {
            url += format!("&port={}", port).as_str();
        }

        if let Some(compact) = config.compact {
            url += format!("&compact={}", if compact { 1 } else { 0 }).as_str();
        }

        debug!("Get tracker announce from: {}", url);

        let response = client
            .get(&url)
            .send()
            .and_then(|mut res| {
                println!("{}", res.status());

                let body = mem::replace(res.body_mut(), ReqwestDecoder::empty());
                body.concat2()
            })
            .and_then(|body| {
                let mut buf = vec![];
                let mut body = std::io::Cursor::new(body);
                std::io::copy(&mut body, &mut buf).unwrap();
                Ok(buf)
            });

        self.state = TorrentProcessState::AnnounceRequestTracker(Box::new(response));
        task::current().notify();
        Ok(Async::NotReady)
    }

    fn announce_response(&mut self, response: Vec<u8>) -> Poll<(), ()> {
        debug!(
            "Tracker response (url encoded): {}",
            percent_encode(&response, SIMPLE_ENCODE_SET).to_string()
        );
        let tracker_announce_response: TrackerAnnounce =
            response.try_into().map_err(|_| ())?;
        debug!("Tracker response parsed: {:#?}", tracker_announce_response);

        self.state = TorrentProcessState::ConnectPeers(tracker_announce_response.peers.unwrap());
        task::current().notify();
        Ok(Async::NotReady)
    }
}

impl Future for TorrentProcessFeature {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.state {
            TorrentProcessState::Done => Ok(Async::Ready(())),
            TorrentProcessState::Announce => self.announce_request(),
            TorrentProcessState::AnnounceRequestTracker(ref mut request) => {
                debug!("receiving");
                let response = try_ready!(request.poll().map_err(|_| ()));
                self.announce_response(response)
            }
            TorrentProcessState::ConnectPeers(ref mut peers) => {
                for peer in peers {
                    let addr = SocketAddr::new(peer.ip.into(), peer.port);
                    debug!("Handshake with {}", addr);
                    let process = self.process.clone();
                    let process_reply = self.process.clone();
                    let (tx, rx) = channel(10);

                    let conntx = tx.clone();

                    let task_keepalive = Interval::new(Instant::now(), Duration::from_secs(10))
                        .for_each(move |_| {
                            let conntx = conntx.clone();
                            conntx
                                .send(Message::KeepAlive)
                                .map(|_| ())
                                .map_err(|_| tokio::timer::Error::shutdown())
                        })
                        .map_err(move |e| error!("Peer {}: interval errored; err={:?}", addr, e));
                    let tcp_stream = TcpStream::connect(&addr)
                        .and_then(move |stream| {
                            let mut buf = vec![];
                            buf.extend_from_slice(&crate::types::HANDSHAKE_PREFIX);
                            buf.extend_from_slice(&process.hash_id);
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

                            if handshake.info_hash != process_reply.hash_id {
                                error!("Peer {}: hash is wrong. Disconnect.", addr);
                                return Err(());
                            }

                            let (writer, reader) = stream.framed(MessageCodec::default()).split();

                            let writer = writer.sink_map_err(|err| error!("{}", err));

                            let sink = rx.forward(writer).inspect(move |(_a, _sink)| {
                                debug!("Peer {}: updated", addr);
                            });
                            tokio::spawn(sink.map(|_| ()));

                            let conn = reader
                                .for_each(move |frame| {
                                    debug!("Peer {}: received message {:?}", addr, frame);
                                    match frame {
                                        Message::KeepAlive => {
                                            let conntx = tx.clone();
                                            tokio::spawn(conntx.send(Message::KeepAlive).map(|_| ()).map_err(|_| ()));
                                        }
                                        _ => (),
                                    }
                                    Ok(())
                                })
                                .map_err(move |err| {
                                    error!("Peer {}: message codec error: {}", addr, err)
                                });

                            tokio::spawn(conn);

                            Ok(())
                        });
                    tokio::spawn(tcp_stream.join(task_keepalive).map(|_| ()));
                }
                Ok(Async::NotReady)
            }
        }
    }
}

pub enum TorrentProcessState {
    Announce,
    AnnounceRequestTracker(Box<dyn Future<Item = Vec<u8>, Error = reqwest::Error> + Send>),
    ConnectPeers(Vec<Peer>),
    Done,
}

pub enum TorrentRequest {
    Add(Arc<TorrentProcess>),
}

const DEFAULT_IP: Ipv4Addr = Ipv4Addr::new(0, 0, 0, 0);

impl RustorrentApp {
    pub fn new(settings: Settings) -> Self {
        let (torrent_request_sender, torrent_request_receiver) = unbounded();
        Self {
            settings: Arc::new(settings),
            processes: Arc::new(RwLock::new(vec![])),
            torrent_request_sender,
            torrent_request_receiver: RefCell::new(Some(torrent_request_receiver)),
        }
    }

    pub fn add_torrent_from_file(&self, filename: impl AsRef<Path>) -> Result<(), ExitFailure> {
        info!("Adding torrent from file: {:?}", filename.as_ref());
        let torrent = parse_torrent(filename).with_context(|_| "cannot parse torrent")?;
        let hash_id = torrent.info_sha1_hash();
        let mut processes = self.processes.write().unwrap();
        let process = Arc::new(TorrentProcess { torrent, hash_id });
        self.torrent_request_sender
            .unbounded_send(TorrentRequest::Add(process.clone()))?;
        processes.push(process.clone());
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), RustorrentError> {
        let config = &self.settings.config;
        let port = config.port.unwrap();
        let ip = config.ipv4.unwrap_or(DEFAULT_IP);

        let addr = SocketAddr::new(IpAddr::V4(ip), port);

        let listener = TcpListener::bind(&addr)?;

        let server = listener
            .incoming()
            .for_each(|socket| {
                let (reader, writer) = socket.split();
                Ok(())
            })
            .map_err(|err| {
                error!("accept error = {:?}", err);
            });

        let mut torrent_request_receiver = self.torrent_request_receiver.borrow_mut();
        let torrent_request_receiver = torrent_request_receiver.take().unwrap();
        let settings = self.settings.clone();
        let torrent_requests = torrent_request_receiver
            .for_each(move |request| {
                info!("adding request!");

                let feature: Box<dyn Future<Item = (), Error = ()> + Send> = match request {
                    TorrentRequest::Add(process) => Box::new(TorrentProcessFeature {
                        settings: settings.clone(),
                        process,
                        state: TorrentProcessState::Announce,
                    }),
                };
                tokio::spawn(feature);
                Ok(())
            })
            .map_err(|e| error!("error = {:?}", e));
        info!("starting run loop");

        let (sender, receiver) = futures::sync::oneshot::channel::<()>();
        let all = lazy(move || {
            let when = Instant::now() + Duration::from_secs(15);
            let task = Delay::new(when)
                .and_then(|_| {
                    info!("time to break!");
                    sender.send(()).unwrap();
                    info!("break sended!");

                    Ok(())
                })
                .map_err(|_| ());
            // tokio::spawn(task);
            server.join3(torrent_requests, task).map(|_| ())
        });

        let receiver = receiver
            .and_then(|_| {
                info!("received the shit");
                Ok(())
            })
            .map(|_| ())
            .map_err(|err| error!("{}", err));
        tokio::run(
            all.select2(receiver)
                .map_err(|_| error!("error"))
                .then(|_| {
                    info!("done");
                    Ok(())
                }),
        );

        Ok(())
    }
}
*/
