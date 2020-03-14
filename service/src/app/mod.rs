use super::*;
use crate::{errors::RsbtError, types::torrent::parse_torrent, PEER_ID};

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

mod accept_connections_loop;
mod connect_to_peer;
mod determine_download_mode;
mod download_events_loop;
pub(crate) mod download_torrent;
mod peer_connection;
mod peer_loop;
mod peer_loop_message;
mod select_new_peer;

use accept_connections_loop::accept_connections_loop;
use connect_to_peer::connect_to_peer;
use determine_download_mode::determine_download_mode;
use download_events_loop::download_events_loop;
use download_torrent::{download_torrent, DownloadTorrentEvent};
use peer_connection::peer_connection;
use peer_loop::peer_loop;
use peer_loop_message::PeerLoopMessage;
use select_new_peer::select_new_peer;

pub struct RsbtApp {
    pub settings: Arc<Settings>,
}

#[derive(Clone)]
pub struct TorrentDownload {
    pub id: usize,
    pub name: String,
    pub active: bool,
    pub process: Arc<TorrentProcess>,
}

#[derive(Debug)]
pub struct TorrentProcess {
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

    pub fn response(self, result: R) -> Result<(), RsbtError> {
        match self {
            RequestResponse::ResponseOnly(response) | RequestResponse::Full { response, .. } => {
                response
                    .send(result)
                    .map_err(|_| RsbtError::FailureReason("Cannot send response".into()))
            }
            RequestResponse::RequestOnly(_) => Ok(()),
        }
    }
}

pub enum RsbtCommand {
    AddTorrent(
        RequestResponse<Vec<u8>, Result<Arc<TorrentProcess>, RsbtError>>,
        String,
    ),
    TorrentHandshake {
        handshake_request: Handshake,
        handshake_sender: oneshot::Sender<Option<Arc<TorrentProcess>>>,
    },
    TorrentList {
        sender: oneshot::Sender<Vec<TorrentDownload>>,
    },
}

pub(crate) struct PeerState {
    peer: Peer,
    state: TorrentPeerState,
    announce_count: usize,
}

#[derive(Debug)]
pub(crate) enum PeerMessage {
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

pub(crate) enum TorrentDownloadMode {
    Normal,
    Final,
}

impl RsbtApp {
    pub fn new(settings: Settings) -> Self {
        let settings = Arc::new(settings);
        Self { settings }
    }

    pub async fn processing_loop(
        &self,
        sender: Sender<RsbtCommand>,
        receiver: Receiver<RsbtCommand>,
    ) -> Result<(), RsbtError> {
        let config = &self.settings.config;

        let listen = config
            .listen
            .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));

        let addr = SocketAddr::new(listen, config.port);

        let download_events = download_events_loop(self.settings.clone(), receiver);

        let accept_incoming_connections = accept_connections_loop(addr, sender.clone());

        if let Err(err) = try_join(accept_incoming_connections, download_events).await {
            return Err(err);
        }

        Ok(())
    }

    pub async fn download<P: AsRef<Path>>(&self, torrent_file: P) -> Result<(), RsbtError> {
        let (mut download_events_sender, download_events_receiver) =
            mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let buf = std::fs::read(torrent_file.as_ref())?;

        download_events_sender
            .send(RsbtCommand::AddTorrent(
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
    F: Future<Output = Result<(), RsbtError>> + Send + 'static,
    M: Fn() -> String + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = f.await {
            error!("{}: {}", message(), e)
        }
    })
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
        if new & (0b1000_0000 >> j) != 0 {
            pieces.push(i * 8 + j);
        }
    }
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

    #[tokio::test]
    async fn check_process_peer_pieces() {}
}
