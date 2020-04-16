use crate::{
    event::{TorrentEvent, TorrentStatisticMessage},
    peer::{request_message, PeerLoopMessage, PeerMessage},
    process::TorrentToken,
    types::{Message, MessageCodec},
    RsbtError,
};
use futures::{future::try_join, prelude::*, StreamExt};
use log::{debug, error};
use std::sync::Arc;
use tokio::{
    net::TcpStream,
    sync::mpsc::{Receiver, Sender},
};
use tokio_util::codec::Framed;
use uuid::Uuid;

pub(crate) async fn peer_loop(
    torrent_process: Arc<TorrentToken>,
    peer_id: Uuid,
    mut sender: Sender<PeerMessage>,
    mut receiver: Receiver<PeerMessage>,
    stream: TcpStream,
    mut statistic_sender: Sender<TorrentStatisticMessage>,
) -> Result<(), RsbtError> {
    let (wtransport, mut rtransport) = Framed::new(stream, MessageCodec).split();

    let mut broker_sender = torrent_process.broker_sender.clone();

    let command_loop_broker_sender = broker_sender.clone();

    let command_loop = async move {
        let mut processor = PeerLoopMessage {
            peer_id,
            command_loop_broker_sender,
            torrent_process: torrent_process.clone(),
            chocked: true,
            interested: false,
            message_count: 0,
            downloading: None,
            piece_length: 0,
            torrent_piece: None,
            wtransport,
            request: None,
            statistic_sender: statistic_sender.clone(),
        };

        while let Some(message) = receiver.next().await {
            debug!("[{}] peer loop received message: {}", peer_id, message);
            match message {
                PeerMessage::Bitfield(pieces) => {
                    processor.wtransport.send(Message::Bitfield(pieces)).await?;
                }
                PeerMessage::Have(piece) => {
                    let piece_index = piece as u32;
                    processor
                        .wtransport
                        .send(Message::Have { piece_index })
                        .await?;
                }
                PeerMessage::Piece {
                    index,
                    begin,
                    block,
                } => {
                    let block_len = block.len() as u64;
                    debug!(
                        "[{}] sending piece {} {} [{}]",
                        peer_id, index, begin, block_len
                    );
                    processor
                        .wtransport
                        .send(Message::Piece {
                            index,
                            begin,
                            block,
                        })
                        .await?;
                    if let Err(err) = statistic_sender
                        .send(TorrentStatisticMessage::Uploaded(block_len))
                        .await
                    {
                        error!("cannot send uploaded statistics: {}", err);
                    }
                }
                PeerMessage::Cancel => {
                    debug!("[{}] cancel download", peer_id);
                    if let Some((index, begin, length)) = processor.request {
                        processor
                            .wtransport
                            .send(Message::Cancel {
                                index,
                                begin,
                                length,
                            })
                            .await?;
                        processor.request = None;
                        processor.downloading = None;
                        processor.torrent_piece = None;
                        processor
                            .command_loop_broker_sender
                            .send(TorrentEvent::PeerPieceCanceled(peer_id))
                            .await?;
                    }
                }
                PeerMessage::Download(piece) => {
                    debug!("[{}] download now piece: {}", peer_id, piece);
                    processor.piece_length = torrent_process.info.sizes(piece).0;
                    processor.downloading = Some(piece);
                    processor.torrent_piece = Some(Vec::with_capacity(processor.piece_length));

                    if processor.chocked {
                        debug!("[{}] send interested message", peer_id);
                        processor.wtransport.send(Message::Interested).await?;
                    } else if let Some(ref torrent_peer_piece) = processor.torrent_piece {
                        let (index, begin, length) =
                            request_message(torrent_peer_piece, piece, processor.piece_length);
                        processor.request = Some((index, begin, length));
                        processor
                            .wtransport
                            .send(Message::Request {
                                index,
                                begin,
                                length,
                            })
                            .await?;
                    }
                }
                PeerMessage::Disconnect => break,
                PeerMessage::Message(message) => {
                    if processor.peer_loop_message(message).await? {
                        break;
                    }
                }
            }
        }

        debug!("[{}] peer loop command exit", peer_id);

        processor.wtransport.close().await?;

        Ok::<(), RsbtError>(())
    };

    let receive_loop = async move {
        while let Some(Ok(message)) = rtransport.next().await {
            sender.send(PeerMessage::Message(message)).await?;
        }

        debug!("[{}] peer loop receive exit", peer_id);

        if let Err(err) = sender.send(PeerMessage::Disconnect).await {
            error!(
                "[{}] cannot send disconnect message to peer: {}",
                peer_id, err
            );
        }

        Ok::<(), RsbtError>(())
    };

    if let Err(err) = try_join(command_loop, receive_loop).await {
        error!("[{}] peer join fail: {}", peer_id, err);
    };

    broker_sender
        .send(TorrentEvent::PeerDisconnect(peer_id))
        .await?;

    debug!("[{}] peer loop exit", peer_id);

    Ok(())
}
