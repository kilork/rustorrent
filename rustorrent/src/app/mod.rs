use super::*;
use crate::{errors::RustorrentError, types::torrent::parse_torrent, PEER_ID};

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
mod download_torrent;
mod peer_connection;
mod peer_loop;
mod peer_loop_message;
mod process_announce;
mod process_peer_announced;
mod process_peer_connected;
mod process_peer_forwarded;
mod process_peer_interested;
mod process_peer_piece;
mod process_peer_piece_canceled;
mod process_peer_piece_downloaded;
mod process_peer_piece_request;
mod process_peer_pieces;
mod process_peer_unchoke;
mod select_new_peer;

use accept_connections_loop::accept_connections_loop;
use connect_to_peer::connect_to_peer;
use determine_download_mode::determine_download_mode;
use download_events_loop::download_events_loop;
use download_torrent::download_torrent;
use peer_connection::peer_connection;
use peer_loop::peer_loop;
use peer_loop_message::PeerLoopMessage;
use process_announce::process_announce;
use process_peer_announced::process_peer_announced;
use process_peer_connected::process_peer_connected;
use process_peer_forwarded::process_peer_forwarded;
use process_peer_interested::process_peer_interested;
use process_peer_piece::process_peer_piece;
use process_peer_piece_canceled::process_peer_piece_canceled;
use process_peer_piece_downloaded::process_peer_piece_downloaded;
use process_peer_piece_request::process_peer_piece_request;
use process_peer_pieces::process_peer_pieces;
use process_peer_unchoke::process_peer_unchoke;
use select_new_peer::select_new_peer;

pub struct RustorrentApp {
    pub settings: Arc<Settings>,
}

#[derive(Debug)]
pub struct TorrentProcess {
    pub id: usize,
    pub filename: String,
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

#[derive(Debug, PartialEq, Hash, Eq)]
pub(crate) struct Block {
    pub piece: u32,
    pub begin: u32,
    pub length: u32,
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

    pub fn response(self, result: R) -> Result<(), RustorrentError> {
        match self {
            RequestResponse::ResponseOnly(response) | RequestResponse::Full { response, .. } => {
                response
                    .send(result)
                    .map_err(|_| RustorrentError::FailureReason("Cannot send response".into()))
            }
            RequestResponse::RequestOnly(_) => Ok(()),
        }
    }
}

pub enum RustorrentCommand {
    AddTorrent(
        RequestResponse<Vec<u8>, Result<Arc<TorrentProcess>, RustorrentError>>,
        String,
    ),
    TorrentHandshake {
        handshake_request: Handshake,
        handshake_sender: oneshot::Sender<Option<Arc<TorrentProcess>>>,
    },
    TorrentList {
        sender: oneshot::Sender<Vec<Arc<TorrentProcess>>>,
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

impl RustorrentApp {
    pub fn new(settings: Settings) -> Self {
        let settings = Arc::new(settings);
        Self { settings }
    }

    pub async fn processing_loop(
        &self,
        sender: Sender<RustorrentCommand>,
        receiver: Receiver<RustorrentCommand>,
    ) -> Result<(), RustorrentError> {
        let config = &self.settings.config;

        let listen = config
            .listen
            .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));

        let addr = SocketAddr::new(listen.into(), config.port);

        let download_events = download_events_loop(self.settings.clone(), receiver);

        let accept_incoming_connections = accept_connections_loop(addr, sender.clone());

        if let Err(err) = try_join(accept_incoming_connections, download_events).await {
            return Err(err);
        }

        Ok(())
    }

    pub async fn download<P: AsRef<Path>>(&self, torrent_file: P) -> Result<(), RustorrentError> {
        let (mut download_events_sender, download_events_receiver) =
            mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let buf = std::fs::read(torrent_file.as_ref())?;

        download_events_sender
            .send(RustorrentCommand::AddTorrent(
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
    F: Future<Output = Result<(), RustorrentError>> + Send + 'static,
    M: Fn() -> String + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = f.await {
            error!("{}: {}", message(), e)
        }
    })
}

#[derive(Debug)]
pub(crate) enum DownloadTorrentEvent {
    Announce(Vec<Peer>),
    PeerAnnounced(Peer),
    PeerConnected(Uuid, TcpStream),
    PeerForwarded(TcpStream),
    PeerConnectFailed(Uuid),
    PeerDisconnect(Uuid),
    PeerPieces(Uuid, Vec<u8>),
    PeerPiece(Uuid, usize),
    PeerUnchoke(Uuid),
    PeerInterested(Uuid),
    PeerPieceDownloaded(Uuid, Vec<u8>),
    PeerPieceCanceled(Uuid),
    PeerPieceRequest {
        peer_id: Uuid,
        index: u32,
        begin: u32,
        length: u32,
    },
}

impl Display for DownloadTorrentEvent {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            DownloadTorrentEvent::PeerPieceDownloaded(uuid, data) => {
                write!(f, "PeerPieceDownloaded({}, [{}])", uuid, data.len())
            }
            _ => write!(f, "{:?}", self),
        }
    }
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
        if new & (0b10000000 >> j) != 0 {
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
