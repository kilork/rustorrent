use futures::lazy;
use std::cell::RefCell;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use exitfailure::ExitFailure;
use failure::{Context, ResultExt};
use futures::future::join_all;
use futures::prelude::*;
use futures::sync::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use log::{error, info};
use sha1::digest::generic_array::{typenum::U20, GenericArray};
use tokio::io;
use tokio::net::TcpListener;
use tokio::prelude::*;

use crate::errors::RustorrentError;
use crate::types::torrent::parse_torrent;
use crate::types::torrent::Torrent;
use crate::types::Settings;

pub struct RustorrentApp {
    pub settings: Settings,
    pub processes: Arc<RwLock<Vec<Arc<TorrentProcess>>>>,
    torrent_request_sender: UnboundedSender<TorrentRequest>,
    torrent_request_receiver: RefCell<Option<UnboundedReceiver<TorrentRequest>>>,
}

pub struct TorrentProcess {
    pub torrent: Torrent,
    pub hash_id: GenericArray<u8, U20>,
}

pub struct TorrentProcessFeature {
    pub process: Arc<TorrentProcess>,
    pub state: TorrentProcessState,
}

impl TorrentProcess {}

impl Future for TorrentProcessFeature {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        info!("torrent is now in state {:?}", self.state);
        match self.state {
            TorrentProcessState::Done => Ok(Async::Ready(())),
            TorrentProcessState::Announce => {
                self.state = TorrentProcessState::Done;
                task::current().notify();
                Ok(Async::NotReady)
            }
        }
    }
}

#[derive(Debug)]
pub enum TorrentProcessState {
    Announce,
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
            settings,
            processes: Arc::new(RwLock::new(vec![])),
            torrent_request_sender,
            torrent_request_receiver: RefCell::new(Some(torrent_request_receiver)),
        }
    }

    pub fn add_torrent_from_file(&self, filename: impl AsRef<Path>) -> Result<(), ExitFailure> {
        let torrent = parse_torrent(filename).with_context(|_| "cannot parse torrent")?;
        let hash_id = torrent.info_sha1_hash();
        let mut processes = self.processes.write().unwrap();
        let process = Arc::new(TorrentProcess { torrent, hash_id });
        let _result = self
            .torrent_request_sender
            .clone()
            .send(TorrentRequest::Add(process.clone()))
            .wait();
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
            .for_each(|socket| Ok(()))
            .map_err(|err| {
                error!("accept error = {:?}", err);
            });

        let mut torrent_request_receiver = self.torrent_request_receiver.borrow_mut();
        let torrent_request_receiver = torrent_request_receiver.take().unwrap();
        let torrent_requests = torrent_request_receiver
            .for_each(move |request| {
                info!("adding request!");
                let feature: Box<dyn Future<Item = (), Error = ()> + Send> = match request {
                    TorrentRequest::Add(process) => Box::new(TorrentProcessFeature {
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
