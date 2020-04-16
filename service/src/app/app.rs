use crate::{
    app::{accept_peer_connection, CurrentTorrents},
    command::{
        Command, CommandAddTorrent, CommandDeleteTorrent, CommandTorrentAction,
        CommandTorrentAnnounce, CommandTorrentDetail, CommandTorrentFileDownload,
        CommandTorrentFiles, CommandTorrentPeers, CommandTorrentPieces,
    },
    event::{torrent_event_loop, TorrentEvent},
    file_download::FileDownloadStream,
    parser::parse_torrent,
    process::{
        find_process_by_id, TorrentProcess, TorrentProcessHeader, TorrentProcessStatus,
        TorrentToken,
    },
    request_response::RequestResponse,
    storage::TorrentStorage,
    types::{
        public::{AnnounceView, FileView, PeerView, TorrentAction, TorrentDownloadView},
        Properties, HANDSHAKE_PREFIX,
    },
    RsbtError, DEFAULT_CHANNEL_BUFFER, PEER_ID, TORRENTS_TOML,
};
use futures::{future::join, prelude::*};
use log::{debug, error};
use sha1::Digest;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs,
    net::TcpListener,
    sync::mpsc::{self, Receiver, Sender},
};

async fn command_loop(app: &mut App, mut events: Receiver<Command>) {
    while let Some(event) = events.next().await {
        match event {
            Command::AddTorrent(request_response) => {
                debug!("add torrent");
                let torrent = app.add_torrent(request_response.request()).await;
                if let Err(err) = request_response.response(torrent) {
                    error!("cannot send response for add torrent: {}", err);
                }
            }
            Command::TorrentHandshake {
                handshake_request,
                handshake_sender,
            } => {
                debug!("searching for a torrent matching the handshake");

                let hash_id = handshake_request.info_hash;

                if handshake_sender
                    .send(
                        app.torrents
                            .iter()
                            .map(|x| &x.process)
                            .find(|x| x.hash_id == hash_id)
                            .cloned(),
                    )
                    .is_err()
                {
                    error!("cannot send handshake, receiver is dropped");
                }
            }
            Command::TorrentList(request_response) => {
                debug!("collecting torrent list");
                let torrents_view = app.torrents.iter().map(TorrentDownloadView::from).collect();
                if let Err(err) = request_response.response(Ok(torrents_view)) {
                    error!("cannot send response for torrent list: {}", err);
                }
            }
            Command::TorrentAction(request_response) => {
                debug!("torrent action");

                let response = app.torrent_action(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for torrent action: {}", err);
                }
            }
            Command::DeleteTorrent(request_response) => {
                debug!("delete torrent");

                let response = app.delete_torrent(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for delete torrent: {}", err);
                }
            }
            Command::TorrentPeers(request_response) => {
                debug!("torrent's peers");
                let response = app.torrent_peers(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for torrent's peers: {}", err);
                }
            }
            Command::TorrentAnnounces(request_response) => {
                debug!("torrent's announces");
                let response = app.torrent_announces(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for torrent's announces: {}", err);
                }
            }
            Command::TorrentFiles(request_response) => {
                debug!("torrent's files");
                let response = app.torrent_files(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for torrent's files: {}", err);
                }
            }
            Command::TorrentFileDownloadHeader(request_response) => {
                debug!("torrent's files");
                let response = app.torrent_file(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for torrent's files: {}", err);
                }
            }
            Command::TorrentFileDownload(request_response) => {
                debug!("torrent's files");
                let response = app.torrent_file_download(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for torrent's files: {}", err);
                }
            }
            Command::TorrentPieces(request_response) => {
                debug!("torrent's pieces");
                let response = app.torrent_pieces(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for torrent's pieces: {}", err);
                }
            }
            Command::TorrentDetail(request_response) => {
                debug!("torrent's detail");
                let response = app.torrent_detail(request_response.request()).await;

                if let Err(err) = request_response.response(response) {
                    error!("cannot send response for torrent's detail: {}", err);
                }
            }
        }
    }

    debug!("download_events_loop done");
}

pub(crate) async fn accept_connections_loop(
    addr: SocketAddr,
    sender: Sender<Command>,
) -> Result<(), RsbtError> {
    debug!("listening on: {}", &addr);
    let mut listener = TcpListener::bind(addr).await?;

    loop {
        let (socket, _) = listener.accept().await?;
        let sender_task = sender.clone();
        tokio::spawn(async move {
            if let Err(err) = accept_peer_connection(socket, sender_task).await {
                format!("peer connection {} failed: {}", addr, err);
            }
        });
    }
}

pub struct App {
    pub properties: Arc<Properties>,
    pub(crate) torrents: Vec<TorrentProcess>,
    pub(crate) id: usize,
}

impl App {
    pub fn new(properties: Properties) -> Self {
        let properties = Arc::new(properties);
        Self {
            properties,
            torrents: vec![],
            id: 0,
        }
    }

    pub async fn processing_loop(
        &mut self,
        sender: Sender<Command>,
        receiver: Receiver<Command>,
    ) -> Result<(), RsbtError> {
        let addr = SocketAddr::new(self.properties.listen, self.properties.port);

        let commands = command_loop(self, receiver);

        let accept_incoming_connections = accept_connections_loop(addr, sender.clone());

        join(accept_incoming_connections, commands).await.0?;

        Ok(())
    }

    pub async fn init_storage(&self) -> Result<CurrentTorrents, RsbtError> {
        let properties = &self.properties;
        if !properties.save_to.exists() {
            fs::create_dir_all(&properties.save_to).await?;
        }
        if !properties.storage.exists() {
            fs::create_dir_all(&properties.storage).await?;
        }

        let torrents_path = properties.config_dir.join(TORRENTS_TOML);

        if torrents_path.is_file() {
            let torrents_toml = fs::read_to_string(torrents_path).await?;
            return Ok(toml::from_str(&torrents_toml)?);
        }

        Ok(Default::default())
    }

    pub async fn download<P: AsRef<Path>>(&mut self, torrent_file: P) -> Result<(), RsbtError> {
        let (mut download_events_sender, download_events_receiver) =
            mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let data = std::fs::read(torrent_file.as_ref())?;

        download_events_sender
            .send(Command::AddTorrent(RequestResponse::RequestOnly(
                CommandAddTorrent {
                    data,
                    filename: torrent_file
                        .as_ref()
                        .file_name()
                        .unwrap_or_default()
                        .to_str()
                        .unwrap_or_default()
                        .into(),
                    state: TorrentProcessStatus::Enabled,
                },
            )))
            .await?;

        self.processing_loop(download_events_sender, download_events_receiver)
            .await
    }

    pub(crate) async fn add_to_current_torrents(
        &mut self,
        torrent_header: TorrentProcessHeader,
    ) -> Result<(), RsbtError> {
        let torrents_toml = self.properties.config_dir.join(TORRENTS_TOML);
        let mut current_torrents: CurrentTorrents = if torrents_toml.exists() {
            toml::from_str(&fs::read_to_string(&torrents_toml).await?)?
        } else {
            Default::default()
        };

        if let Some(current_torrent_header) = current_torrents
            .torrents
            .iter_mut()
            .find(|x| x.file == torrent_header.file)
        {
            *current_torrent_header = torrent_header;
        } else {
            current_torrents.torrents.push(torrent_header);
        }

        fs::write(torrents_toml, toml::to_string(&current_torrents)?).await?;

        Ok(())
    }

    pub(crate) async fn remove_from_current_torrents(
        &mut self,
        torrent_header: TorrentProcessHeader,
    ) -> Result<(), RsbtError> {
        let torrents_toml = self.properties.config_dir.join(TORRENTS_TOML);
        let mut current_torrents: CurrentTorrents = if torrents_toml.exists() {
            toml::from_str(&fs::read_to_string(&torrents_toml).await?)?
        } else {
            Default::default()
        };

        if let Some(position) = current_torrents
            .torrents
            .iter()
            .position(|x| x.file == torrent_header.file)
        {
            current_torrents.torrents.remove(position);
            fs::write(torrents_toml, toml::to_string(&current_torrents)?).await?;
        }

        Ok(())
    }

    async fn add_torrent(
        &mut self,
        request: &CommandAddTorrent,
    ) -> Result<TorrentProcess, RsbtError> {
        let CommandAddTorrent {
            data,
            filename,
            state,
        } = request;
        debug!("we need to download {:?}", filename);
        let filepath = PathBuf::from(&filename);
        let name = filepath.file_stem().unwrap().to_string_lossy().into_owned();

        let torrent = parse_torrent(data)?;
        let hash_id = torrent.info_sha1_hash();
        let info = torrent.info()?;

        debug!("torrent size: {}", info.len());
        debug!("piece length: {}", info.piece_length);
        debug!("total pieces: {}", info.pieces.len());

        let mut handshake = vec![];
        handshake.extend_from_slice(&HANDSHAKE_PREFIX);
        handshake.extend_from_slice(&hash_id);
        handshake.extend_from_slice(&PEER_ID);

        let (broker_sender, broker_receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        self.id += 1;

        let torrent_token = Arc::new(TorrentToken {
            info,
            hash_id,
            torrent,
            handshake,
            broker_sender,
        });

        let torrent_storage = TorrentStorage::new(
            self.properties.clone(),
            filename.clone(),
            torrent_token.clone(),
        )
        .await?;

        let torrent_header = TorrentProcessHeader {
            file: filename.clone(),
            state: state.clone(),
        };
        let storage_state_watch = torrent_storage.receiver.clone();
        tokio::spawn(torrent_event_loop(
            self.properties.clone(),
            torrent_storage,
            torrent_token.clone(),
            broker_receiver,
        ));

        let (statistics_request_response, statistics_receiver) = RequestResponse::new(());

        torrent_token
            .broker_sender
            .clone()
            .send(TorrentEvent::Subscribe(statistics_request_response))
            .await?;

        let statistics_watch = statistics_receiver.await?;

        let torrent_process = TorrentProcess {
            id: self.id,
            name,
            header: torrent_header.clone(),
            process: torrent_token.clone(),
            properties: self.properties.clone(),
            storage_state_watch,
            statistics_watch,
        };

        self.add_to_current_torrents(torrent_header).await?;

        self.torrents.push(torrent_process.clone());

        if state == &TorrentProcessStatus::Enabled {
            debug!("sending activation event");
            let (enable_request, response) = RequestResponse::new(());
            torrent_token
                .broker_sender
                .clone()
                .send(TorrentEvent::Enable(enable_request))
                .await?;
            response.await??;
            debug!("sending activation event done");
        }

        Ok(torrent_process)
    }

    async fn torrent_action(&mut self, request: &CommandTorrentAction) -> Result<(), RsbtError> {
        let id = request.id;

        let torrent_header = if let Some(torrent) = self.torrents.iter_mut().find(|x| x.id == id) {
            match request.action {
                TorrentAction::Enable => torrent.enable().await,
                TorrentAction::Disable => torrent.disable().await,
            }?;
            Ok(torrent.header.clone())
        } else {
            Err(RsbtError::TorrentNotFound(id))
        }?;
        self.add_to_current_torrents(torrent_header).await
    }

    async fn delete_torrent(&mut self, request: &CommandDeleteTorrent) -> Result<(), RsbtError> {
        let id = request.id;

        if let Some(torrent_index) = self.torrents.iter().position(|x| x.id == id) {
            let mut torrent_header = None;
            if let Some(torrent) = self.torrents.get_mut(torrent_index) {
                torrent.disable().await?;
                torrent.delete(request.files).await?;
                torrent_header = Some(torrent.header.clone());
            }
            if let Some(torrent_header) = torrent_header.take() {
                self.remove_from_current_torrents(torrent_header).await?;
            }

            self.torrents.remove(torrent_index);

            Ok(())
        } else {
            Err(RsbtError::TorrentNotFound(id))
        }
    }

    async fn torrent_peers(
        &mut self,
        request: &CommandTorrentPeers,
    ) -> Result<Vec<PeerView>, RsbtError> {
        let torrent = find_process_by_id(&self.torrents, request.id)?;
        torrent.peers().await
    }

    async fn torrent_announces(
        &mut self,
        request: &CommandTorrentAnnounce,
    ) -> Result<Vec<AnnounceView>, RsbtError> {
        let torrent = find_process_by_id(&self.torrents, request.id)?;
        torrent.announces().await
    }

    async fn torrent_files(
        &mut self,
        request: &CommandTorrentFiles,
    ) -> Result<Vec<FileView>, RsbtError> {
        let torrent = find_process_by_id(&self.torrents, request.id)?;
        torrent.files().await
    }

    async fn torrent_file(
        &mut self,
        request: &CommandTorrentFileDownload,
    ) -> Result<FileView, RsbtError> {
        let CommandTorrentFileDownload { id, file_id, range } = request;
        let torrent = find_process_by_id(&self.torrents, *id)?;
        let files = torrent.files().await?;
        let file_id = *file_id;
        files
            .into_iter()
            .find(|x| x.id == file_id)
            .ok_or_else(|| RsbtError::TorrentFileNotFound(file_id))
            .and_then(|file| {
                if let Some(range) = range {
                    if range.end > file.size {
                        return Err(RsbtError::TorrentFileRangeInvalid {
                            file_size: file.size,
                        });
                    }
                }
                Ok(file)
            })
    }

    async fn torrent_detail(
        &mut self,
        request: &CommandTorrentDetail,
    ) -> Result<TorrentDownloadView, RsbtError> {
        let torrent = find_process_by_id(&self.torrents, request.id)?;
        Ok(torrent.into())
    }

    async fn torrent_pieces(
        &mut self,
        request: &CommandTorrentPieces,
    ) -> Result<Vec<u8>, RsbtError> {
        let torrent = find_process_by_id(&self.torrents, request.id)?;
        Ok(torrent.storage_state_watch.borrow().downloaded.to_vec())
    }

    async fn torrent_file_download(
        &mut self,
        request: &CommandTorrentFileDownload,
    ) -> Result<FileDownloadStream, RsbtError> {
        let CommandTorrentFileDownload { id, file_id, range } = request;
        let torrent = find_process_by_id(&self.torrents, *id)?;
        torrent.download_file(*file_id, range.clone()).await
    }
}
