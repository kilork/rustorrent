use std::cell::RefCell;
use std::convert::TryInto;
use std::mem;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use exitfailure::ExitFailure;
use failure::{Context, ResultExt};
use futures::future::join_all;
use futures::lazy;
use futures::prelude::*;
use futures::sync::mpsc::{channel, Receiver, Sender};
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::try_ready;
use log::{debug, error, info};
use percent_encoding::{percent_encode, percent_encode_byte, SIMPLE_ENCODE_SET};
use reqwest::r#async::{Client, Decoder};
use tokio::io;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

use crate::errors::RustorrentError;
use crate::types::message::{Message, MessageCodec, MessageCodecError};
use crate::types::peer::Peer;
use crate::types::torrent::parse_torrent;
use crate::types::torrent::{Torrent, TrackerAnnounceResponse};
use crate::types::Settings;

pub struct RustorrentApp {
    pub settings: Arc<Settings>,
    pub processes: Arc<RwLock<Vec<Arc<TorrentProcess>>>>,
    torrent_request_sender: UnboundedSender<TorrentRequest>,
    torrent_request_receiver: RefCell<Option<UnboundedReceiver<TorrentRequest>>>,
}

pub struct TorrentProcess {
    pub torrent: Torrent,
    pub hash_id: [u8; 20],
}

impl TorrentProcess {}

const PEER_ID: [u8; 20] = *b"-rs0001-zzzzxxxxyyyy";

fn url_encode(data: &[u8]) -> String {
    data.iter()
        .map(|&x| percent_encode_byte(x))
        .collect::<String>()
}

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

                let body = mem::replace(res.body_mut(), Decoder::empty());
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
        let tracker_announce_response: TrackerAnnounceResponse =
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
                        .and_then(move |(stream, buf)| {
                            debug!(
                                "Handshake reply from {} (url encoded): {} (len: {})",
                                addr,
                                percent_encode(&buf, SIMPLE_ENCODE_SET).to_string(),
                                buf.len()
                            );

                            let (writer, reader) = stream.framed(MessageCodec::default()).split();
                            let (tx, rx) = channel(10);

                            let writer = writer.sink_map_err(|err| error!("{}", err));

                            let sink = rx.forward(writer);
                            tokio::spawn(sink.map(|_| ()));

                            let mut conntx = tx.clone();
                            let conn = reader
                                .for_each(move |frame| {
                                    debug!("Peer {}: received message {:?}", addr, frame);
                                    match frame {
                                        Message::KeepAlive => {
                                            if let Err(err) = conntx.start_send(Message::KeepAlive) {
                                                error!("send exception: {}", err);
                                            }
                                        }
                                        _ => ()
                                    }
                                    Ok(())
                                })
                                .map_err(|err| error!("message codec error: {}", err));

                            tokio::spawn(conn);

                            Ok(())
                        })
                        .map_err(move |err| error!("Peer connect to {} failed: {}", addr, err));
                    tokio::spawn(tcp_stream);
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

        tokio::run(server.join(torrent_requests).map(|_| ()));

        Ok(())
    }
}
