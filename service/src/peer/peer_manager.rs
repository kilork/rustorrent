use crate::{
    announce::{AnnounceManager, AnnounceManagerMessage},
    event::{TorrentDownloadMode, TorrentEvent, TorrentEventQueryPiece, TorrentStatisticMessage},
    event_loop::EventLoop,
    peer::{connect_to_peer, peer_loop, PeerMessage, PeerState, TorrentPeerState},
    piece::{collect_pieces_and_update, match_pieces},
    process::TorrentToken,
    request_response::RequestResponse,
    spawn_and_log_error,
    storage::TorrentStorage,
    types::Peer,
    RsbtError, DEFAULT_CHANNEL_BUFFER,
};
use flat_storage::{bit_by_index, index_in_bitarray};
use futures::future::AbortHandle;
use log::{debug, error};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::{
    net::TcpStream,
    sync::mpsc::{self, Sender},
};
use uuid::Uuid;

pub(crate) struct PeerManager {
    pub(crate) announce_manager: EventLoop<AnnounceManagerMessage, AnnounceManager>,
    pub(crate) torrent_storage: TorrentStorage,
    pub(crate) torrent_process: Arc<TorrentToken>,
    pub(crate) peer_states: HashMap<Uuid, PeerState>,
    pub(crate) mode: TorrentDownloadMode,
    pub(crate) active: bool,
    pub(crate) announce_abort_handle: Option<AbortHandle>,
    pub(crate) awaiting_for_piece:
        HashMap<usize, Vec<RequestResponse<TorrentEventQueryPiece, Result<Vec<u8>, RsbtError>>>>,
    pub(crate) statistic_sender: Sender<TorrentStatisticMessage>,
}

impl PeerManager {
    pub(crate) async fn peer_announced(&mut self, peer: Peer) -> Result<(), RsbtError> {
        let torrent_process = self.torrent_process.clone();
        let mut peer_states_iter = self.peer_states.iter_mut();
        let peer_err = peer.clone();
        if let Some((peer_id, existing_peer)) = peer_states_iter.find(|x| x.1.peer == peer) {
            let peer_id = *peer_id;
            match existing_peer.state {
                TorrentPeerState::Idle => {
                    let handler = spawn_and_log_error(
                        connect_to_peer(torrent_process, peer_id, peer),
                        move || {
                            format!("connect to existing peer {} {:?} failed", peer_id, peer_err)
                        },
                    );
                    existing_peer.state = TorrentPeerState::Connecting(handler);
                }
                TorrentPeerState::Connected { .. } => {
                    existing_peer.announce_count += 1;
                }
                _ => (),
            }
        } else {
            let peer_id = Uuid::new_v4();
            let torrent_process_on_failure = torrent_process.clone();
            self.peer_states.insert(
                peer_id,
                PeerState {
                    peer: peer.clone(),
                    state: TorrentPeerState::Connecting(tokio::spawn(async move {
                        if let Err(err) = connect_to_peer(torrent_process, peer_id, peer).await {
                            error!(
                                "[{}] connect to new peer {:?} failed: {}",
                                peer_id, peer_err, err
                            );
                            if let Err(err) = torrent_process_on_failure
                                .broker_sender
                                .clone()
                                .send(TorrentEvent::PeerConnectFailed(peer_id))
                                .await
                            {
                                error!("[{}] cannot send peer connect failed: {}", peer_id, err);
                            }
                        }
                    })),
                    announce_count: 0,
                },
            );
        };

        Ok(())
    }

    pub(crate) async fn peer_forwarded(&mut self, stream: TcpStream) -> Result<(), RsbtError> {
        let peer_id = Uuid::new_v4();
        debug!("[{}] peer connection forwarded", peer_id);

        let peer_addr = stream.peer_addr()?;

        let peer: Peer = peer_addr.into();

        let (mut sender, receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        self.peer_states.insert(
            peer_id,
            PeerState {
                peer: peer.clone(),
                state: TorrentPeerState::Connected {
                    chocked: true,
                    interested: false,
                    downloading_piece: None,
                    downloading_since: None,
                    downloaded: 0,
                    uploaded: 0,
                    pieces: vec![],
                    sender: sender.clone(),
                },
                announce_count: 0,
            },
        );

        {
            let downloaded = self.torrent_storage.receiver.borrow().downloaded.clone();
            if !downloaded.is_empty() {
                sender.send(PeerMessage::Bitfield(downloaded)).await?;
            }
        }

        let _ = spawn_and_log_error(
            peer_loop(
                self.torrent_process.clone(),
                peer_id,
                sender,
                receiver,
                stream,
                self.statistic_sender.clone(),
            ),
            move || format!("[{}] peer loop failed", peer_id),
        );

        Ok(())
    }

    pub(crate) async fn peer_connected(
        &mut self,
        peer_id: Uuid,
        stream: TcpStream,
    ) -> Result<(), RsbtError> {
        debug!("[{}] peer connection initiated", peer_id);

        if let Some(existing_peer) = self.peer_states.get_mut(&peer_id) {
            let (sender, receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

            let _ = spawn_and_log_error(
                peer_loop(
                    self.torrent_process.clone(),
                    peer_id,
                    sender.clone(),
                    receiver,
                    stream,
                    self.statistic_sender.clone(),
                ),
                move || format!("[{}] existing peer loop failed", peer_id),
            );

            existing_peer.state = TorrentPeerState::Connected {
                chocked: true,
                interested: false,
                downloading_piece: None,
                downloading_since: None,
                downloaded: 0,
                uploaded: 0,
                pieces: vec![],
                sender,
            };
        }

        Ok(())
    }

    pub(crate) async fn select_new_peer(
        &mut self,
        new_pieces: &[usize],
        peer_id: Uuid,
    ) -> Result<(), RsbtError> {
        for &new_piece in new_pieces {
            if let TorrentDownloadMode::Normal = self.mode {
                let any_peer_downloading = self.peer_states.values().any(|x| match x.state {
                    TorrentPeerState::Connected {
                        downloading_piece, ..
                    } => downloading_piece == Some(new_piece),
                    _ => false,
                });
                if any_peer_downloading {
                    continue;
                }
            }

            if let Some(existing_peer) = self.peer_states.get_mut(&peer_id) {
                if let TorrentPeerState::Connected {
                    ref mut downloading_piece,
                    ref mut downloading_since,
                    ref mut sender,
                    ..
                } = existing_peer.state
                {
                    if downloading_piece.is_none() {
                        *downloading_piece = Some(new_piece);
                        *downloading_since = Some(Instant::now());
                        sender.send(PeerMessage::Download(new_piece)).await?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Peer reveived message Have.
    pub(crate) async fn peer_piece(
        &mut self,
        peer_id: Uuid,
        peer_piece: usize,
    ) -> Result<(), RsbtError> {
        debug!("[{}] peer piece", peer_id);

        let new_pieces = if let Some(existing_peer) = self.peer_states.get_mut(&peer_id) {
            match existing_peer.state {
                TorrentPeerState::Connected { .. } => {
                    let mut downloadable = vec![];
                    let (index, bit) = index_in_bitarray(peer_piece);
                    match_pieces(
                        &mut downloadable,
                        &self.torrent_storage.receiver.borrow().downloaded,
                        index,
                        bit,
                    );
                    downloadable
                }
                TorrentPeerState::Idle | TorrentPeerState::Connecting(_) => {
                    error!(
                        "[{}] cannot process peer piece: wrong state: {:?}",
                        peer_id, existing_peer.state
                    );
                    vec![]
                }
            }
        } else {
            vec![]
        };

        self.select_new_peer(&new_pieces, peer_id).await?;

        Ok(())
    }

    pub(crate) async fn peer_pieces(
        &mut self,
        peer_id: Uuid,
        peer_pieces: Vec<u8>,
    ) -> Result<(), RsbtError> {
        debug!("[{}] process peer pieces", peer_id);

        let new_pieces = if let Some(existing_peer) = self.peer_states.get_mut(&peer_id) {
            match &mut existing_peer.state {
                TorrentPeerState::Connected { pieces, .. } => collect_pieces_and_update(
                    pieces,
                    &peer_pieces,
                    &self.torrent_storage.receiver.borrow().downloaded,
                ),
                TorrentPeerState::Idle | TorrentPeerState::Connecting(_) => {
                    error!(
                        "[{}] cannot process peer pieces: wrong state: {:?}",
                        peer_id, existing_peer.state
                    );
                    vec![]
                }
            }
        } else {
            vec![]
        };

        self.select_new_peer(&new_pieces, peer_id).await?;

        Ok(())
    }

    pub(crate) async fn peer_unchoke(&mut self, peer_id: Uuid) -> Result<(), RsbtError> {
        debug!("[{}] process peer unchoke", peer_id);

        if let Some(TorrentPeerState::Connected {
            ref mut chocked, ..
        }) = self.peer_states.get_mut(&peer_id).map(|x| &mut x.state)
        {
            *chocked = false;
        }

        Ok(())
    }

    pub(crate) async fn peer_interested(&mut self, peer_id: Uuid) -> Result<(), RsbtError> {
        debug!("[{}] process peer interested", peer_id);

        if let Some(TorrentPeerState::Connected {
            ref mut interested, ..
        }) = self.peer_states.get_mut(&peer_id).map(|x| &mut x.state)
        {
            *interested = true;
        }

        Ok(())
    }

    pub(crate) async fn peer_piece_canceled(&mut self, peer_id: Uuid) -> Result<(), RsbtError> {
        debug!("[{}] peer piece downloaded", peer_id);

        let new_pieces = if let Some(existing_peer) = self.peer_states.get_mut(&peer_id) {
            if let TorrentPeerState::Connected {
                ref pieces,
                ref mut downloading_piece,
                ref mut downloading_since,
                ..
            } = existing_peer.state
            {
                *downloading_piece = None;
                *downloading_since = None;
                let mut downloadable = vec![];
                for (i, &a) in pieces.iter().enumerate() {
                    match_pieces(
                        &mut downloadable,
                        &self.torrent_storage.receiver.borrow().downloaded,
                        i,
                        a,
                    );
                }
                downloadable
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        };

        self.select_new_peer(&new_pieces, peer_id).await?;

        Ok(())
    }

    pub(crate) async fn peer_piece_downloaded(
        &mut self,
        peer_id: Uuid,
        piece: Vec<u8>,
    ) -> Result<(), RsbtError> {
        debug!("[{}] peer piece downloaded", peer_id);

        let (index, new_pieces) = if let Some(existing_peer) = self.peer_states.get_mut(&peer_id) {
            if let TorrentPeerState::Connected {
                ref pieces,
                ref mut downloading_piece,
                ref mut downloading_since,
                ref mut downloaded,
                ..
            } = existing_peer.state
            {
                *downloaded += piece.len();
                if let (Some(index), Some(_since)) =
                    (downloading_piece.take(), downloading_since.take())
                {
                    self.torrent_storage.save(index, piece.to_vec()).await?;

                    let mut downloadable = vec![];
                    for (i, &a) in pieces.iter().enumerate() {
                        match_pieces(
                            &mut downloadable,
                            &self.torrent_storage.receiver.borrow().downloaded,
                            i,
                            a,
                        );
                    }
                    (index, downloadable)
                } else {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        };

        for (peer_id, peer_state) in self
            .peer_states
            .iter_mut()
            .filter(|(&key, _)| key != peer_id)
        {
            if let TorrentPeerState::Connected {
                ref mut sender,
                ref pieces,
                ref mut downloading_piece,
                ..
            } = peer_state.state
            {
                let peer_already_have_piece = bit_by_index(index, pieces).is_some();
                if peer_already_have_piece {
                    continue;
                }
                debug!("[{}] sending Have {}", peer_id, index);
                if let Err(err) = sender.send(PeerMessage::Have(index)).await {
                    error!(
                        "[{}] cannot send Have to {:?}: {}",
                        peer_id, peer_state.peer, err
                    );
                };

                let peer_downloads_same_piece = *downloading_piece == Some(index);
                if peer_downloads_same_piece {
                    if let Err(err) = sender.send(PeerMessage::Cancel).await {
                        error!(
                            "[{}] cannot send Have to {:?}: {}",
                            peer_id, peer_state.peer, err
                        );
                    };
                }
            }
        }

        self.select_new_peer(&new_pieces, peer_id).await?;

        if let Some(awaiters) = self.awaiting_for_piece.remove(&index) {
            for awaiter in awaiters {
                let waker = awaiter.request().waker.lock().unwrap().take();
                if let Err(err) = awaiter.response(Ok(piece.to_vec())) {
                    error!("cannot send to awaiter: {}", err);
                }
                if let Some(waker) = waker {
                    waker.wake();
                }
            }
        }

        Ok(())
    }

    pub(crate) async fn peer_piece_request(
        &mut self,
        peer_id: Uuid,
        index: u32,
        begin: u32,
        length: u32,
    ) -> Result<(), RsbtError> {
        if let Some(TorrentPeerState::Connected {
            ref mut sender,
            ref mut uploaded,
            ..
        }) = self.peer_states.get_mut(&peer_id).map(|x| &mut x.state)
        {
            if let Some(piece) = self.torrent_storage.load(index as usize).await? {
                *uploaded += length as usize;
                let block =
                    piece.as_ref()[begin as usize..(begin as usize + length as usize)].to_vec();
                sender
                    .send(PeerMessage::Piece {
                        index,
                        begin,
                        block,
                    })
                    .await?;
            }
        }
        Ok(())
    }

    pub(crate) fn update_download_mode(&mut self, peer_id: Uuid) {
        let pieces_left = self.torrent_storage.receiver.borrow().pieces_left;

        let connected_count = self
            .peer_states
            .values()
            .filter(|x| match x.state {
                TorrentPeerState::Connected { .. } => true,
                _ => false,
            })
            .count();

        let final_mode = (pieces_left as usize) < connected_count;

        self.mode = if final_mode {
            debug!("[{}] select piece in final mode", peer_id);
            TorrentDownloadMode::Final
        } else {
            debug!("[{}] select piece in normal mode", peer_id);
            TorrentDownloadMode::Normal
        };
    }

    pub(crate) async fn enable(&mut self) -> Result<(), RsbtError> {
        self.announce_manager.start().await?;

        Ok(())
    }

    pub(crate) async fn disable(&mut self) -> Result<(), RsbtError> {
        self.announce_manager.stop().await?;

        Ok(())
    }

    pub(crate) async fn quit(&mut self) -> Result<(), RsbtError> {
        if let Some(_announce_manager) = self.announce_manager.quit().await? {
            debug!("successfully exited announce manager");
        }

        Ok(())
    }
}
