use super::*;
use crate::app::download_torrent::TorrentStatisticMessage;

pub(crate) struct PeerLoopMessage {
    pub(crate) torrent_process: Arc<TorrentProcess>,
    pub(crate) message_count: usize,
    pub(crate) chocked: bool,
    pub(crate) interested: bool,
    pub(crate) peer_id: Uuid,
    pub(crate) command_loop_broker_sender: Sender<DownloadTorrentEvent>,
    pub(crate) downloading: Option<usize>,
    pub(crate) torrent_piece: Option<Vec<u8>>,
    pub(crate) piece_length: usize,
    pub(crate) wtransport: SplitSink<Framed<TcpStream, MessageCodec>, Message>,
    pub(crate) request: Option<(u32, u32, u32)>,
    pub(crate) statistic_sender: Sender<TorrentStatisticMessage>,
}

impl PeerLoopMessage {
    pub(crate) async fn bitfield(&mut self, pieces: Vec<u8>) -> Result<bool, RsbtError> {
        let peer_id = self.peer_id;
        if self.message_count != 1 {
            error!(
                "[{}] wrong message sequence for peer: bitfield message must be first message",
                peer_id
            );
            return Ok(true);
        }
        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerPieces(peer_id, pieces))
            .await?;

        Ok(false)
    }

    pub(crate) async fn have(&mut self, piece_index: usize) -> Result<bool, RsbtError> {
        let peer_id = self.peer_id;

        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerPiece(peer_id, piece_index))
            .await?;

        Ok(false)
    }

    pub(crate) async fn unchoke(&mut self) -> Result<bool, RsbtError> {
        self.chocked = false;

        let peer_id = self.peer_id;
        debug!("[{}] unchocked", peer_id);

        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerUnchoke(self.peer_id))
            .await?;

        debug!(
            "[{}] send DownloadTorrentEvent::PeerUnchoke message",
            peer_id
        );

        debug!(
            "[{}] checking piece progress: {:?}",
            peer_id, self.downloading
        );
        if let Some(piece) = self.downloading {
            if let Some(ref torrent_peer_piece) = self.torrent_piece {
                let (index, begin, length) =
                    request_message(torrent_peer_piece, piece, self.piece_length);
                self.request = Some((index, begin, length));
                self.wtransport
                    .send(Message::Request {
                        index,
                        begin,
                        length,
                    })
                    .await?;
            }
        }

        Ok(false)
    }

    pub(crate) async fn piece(
        &mut self,
        index: u32,
        begin: u32,
        block: Vec<u8>,
    ) -> Result<bool, RsbtError> {
        let peer_id = self.peer_id;
        if let Err(err) = self
            .statistic_sender
            .send(TorrentStatisticMessage::Downloaded(block.len() as u64))
            .await
        {
            error!("cannot send downloaded statistics: {}", err);
        }

        if let Some(piece) = self.downloading {
            if piece as u32 != index {
                error!(
                    "[{}] abnormal piece message {} for peer, expected {}",
                    peer_id, index, piece
                );
                return Ok(false);
            }
            if let Some(ref mut torrent_peer_piece) = self.torrent_piece {
                if torrent_peer_piece.len() != begin as usize {
                    error!(
                            "[{}] abnormal piece message for peer piece {}, expected begin {} but got {}",
                            peer_id, piece, torrent_peer_piece.len(), begin,
                        );
                    return Ok(false);
                }

                torrent_peer_piece.extend(block);

                use std::cmp::Ordering;
                match self.piece_length.cmp(&torrent_peer_piece.len()) {
                    Ordering::Greater => {
                        let (index, begin, length) =
                            request_message(torrent_peer_piece, piece, self.piece_length);
                        self.request = Some((index, begin, length));
                        self.wtransport
                            .send(Message::Request {
                                index,
                                begin,
                                length,
                            })
                            .await?;
                    }
                    Ordering::Equal => {
                        let control_piece = &self.torrent_process.info.pieces[piece];

                        let sha1: types::info::Piece =
                            Sha1::digest(torrent_peer_piece.as_slice())[..].try_into()?;
                        if sha1 != *control_piece {
                            error!("[{}] piece sha1 failure", peer_id);
                        }

                        self.downloading = None;
                        self.command_loop_broker_sender
                            .send(DownloadTorrentEvent::PeerPieceDownloaded(
                                peer_id,
                                self.torrent_piece.take().unwrap(),
                            ))
                            .await?;
                    }
                    _ => {
                        error!(
                            "[{}] wrong piece length: {} {}",
                            peer_id,
                            piece,
                            torrent_peer_piece.len()
                        );
                        return Ok(false);
                    }
                }
            }
        } else {
            error!("[{}] abnormal piece message {} for peer", peer_id, index);
        }

        Ok(false)
    }

    pub(crate) async fn request(
        &mut self,
        index: u32,
        begin: u32,
        length: u32,
    ) -> Result<bool, RsbtError> {
        let peer_id = self.peer_id;

        if !self.interested {
            error!("[{}] peer requested data without unchoke", peer_id);
            return Ok(true);
        }

        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerPieceRequest {
                peer_id,
                index,
                begin,
                length,
            })
            .await?;

        Ok(false)
    }

    pub(crate) async fn interested(&mut self) -> Result<bool, RsbtError> {
        self.interested = true;
        self.command_loop_broker_sender
            .send(DownloadTorrentEvent::PeerInterested(self.peer_id))
            .await?;

        Ok(false)
    }

    pub(crate) async fn peer_loop_message(&mut self, message: Message) -> Result<bool, RsbtError> {
        let peer_id = self.peer_id;
        debug!("[{}] message {}", peer_id, message);
        self.message_count += 1;
        match message {
            Message::Bitfield(pieces) => {
                return self.bitfield(pieces).await;
            }
            Message::Have { piece_index } => {
                return self.have(piece_index as usize).await;
            }
            Message::Unchoke => {
                return self.unchoke().await;
            }
            Message::Interested => {
                return self.interested().await;
            }
            Message::Piece {
                index,
                begin,
                block,
            } => {
                return self.piece(index, begin, block).await;
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                return self.request(index, begin, length).await;
            }
            _ => debug!("[{}] unhandled message: {}", peer_id, message),
        }

        Ok(false)
    }
}
