use crate::request_response::RequestResponse;
use crate::RsbtError;
use crate::{
    event::{TorrentEvent, TorrentEventQueryPiece},
    file_download::FileDownloadState,
    process::TorrentToken,
};
use bytes::Bytes;
use futures::prelude::*;
use log::{debug, error};
use std::{
    io::Read,
    ops::Range,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

#[derive(Debug)]
pub struct FileDownloadStream {
    pub name: String,
    pub file_size: usize,
    pub size: usize,
    pub left: usize,
    pub piece: usize,
    pub piece_offset: usize,
    pub range: Option<Range<usize>>,
    pub(crate) torrent_process: Arc<TorrentToken>,
    pub(crate) state: FileDownloadState,
    pub(crate) waker: Arc<Mutex<Option<Waker>>>,
}

impl Stream for FileDownloadStream {
    type Item = Result<Bytes, RsbtError>;
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.left == 0 {
            self.get_mut().left = 0;
            Poll::Ready(None)
        } else {
            debug!("going to poll stream");
            let mut that = self.as_mut();
            {
                let mut waker = that.waker.lock().unwrap();
                *waker = Some(cx.waker().clone());
            }
            debug!("starting loop");

            loop {
                match &mut that.state {
                    FileDownloadState::Idle => {
                        debug!("idle state: send message");
                        let (request_response, receiver) =
                            RequestResponse::new(TorrentEventQueryPiece {
                                piece: that.piece,
                                waker: that.waker.clone(),
                            });

                        let torrent_process = that.torrent_process.clone();
                        let future = async move {
                            torrent_process
                                .broker_sender
                                .clone()
                                .send(TorrentEvent::QueryPiece(request_response))
                                .map_err(RsbtError::from)
                                .await
                        };
                        let sender = future.boxed();

                        that.state = FileDownloadState::SendQueryPiece(sender, Some(receiver));
                    }
                    FileDownloadState::SendQueryPiece(ref mut sender, receiver) => {
                        debug!("send query state: poll");
                        match sender.as_mut().poll(cx) {
                            Poll::Ready(Ok(())) => {
                                debug!("send query piece: ok");
                                that.state =
                                    FileDownloadState::ReceiveQueryPiece(receiver.take().unwrap())
                            }
                            Poll::Ready(Err(err)) => {
                                error!("send query piece: err: {}", err);
                                return Poll::Ready(Some(Err(err)));
                            }
                            Poll::Pending => {
                                debug!("send query piece: pending");
                                return Poll::Pending;
                            }
                        }
                    }
                    FileDownloadState::ReceiveQueryPiece(receiver) => {
                        debug!("receive query state: poll");
                        match receiver.poll_unpin(cx) {
                            Poll::Ready(Ok(Ok(data))) => {
                                debug!("receive query piece: received data");
                                let remains = data.len() - that.piece_offset;
                                let size = if remains < that.left {
                                    remains
                                } else {
                                    that.left
                                };
                                let out = &data[that.piece_offset..that.piece_offset + size];
                                that.left -= size;
                                that.piece += 1;
                                that.piece_offset = 0;
                                that.state = FileDownloadState::Idle;
                                debug!("receive query piece: return data {}", out.len());
                                return Poll::Ready(Some(Ok(Bytes::from(out.to_owned()))));
                            }
                            Poll::Ready(Ok(Err(err))) => {
                                error!("receive query piece: received err: {}", err);
                                return Poll::Ready(Some(Err(err)));
                            }
                            Poll::Ready(Err(err)) => {
                                error!("receive query piece: received send err: {}", err);
                                return Poll::Ready(Some(Err(err.into())));
                            }
                            Poll::Pending => {
                                debug!("receive query piece: pending");
                                return Poll::Pending;
                            }
                        }
                    }
                }
            }
        }
    }
}
