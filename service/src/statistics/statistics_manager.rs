use crate::{
    event::{TorrentEvent, TorrentStatisticMessage},
    event_loop::{EventLoopRunner, EventLoopSender},
    storage::TorrentStorage,
    types::public::TorrentDownloadState,
    RsbtError,
};
use async_trait::async_trait;
use log::error;
use tokio::sync::watch;

pub(crate) struct StatisticsManager {
    sender: Option<EventLoopSender<TorrentStatisticMessage, TorrentEvent>>,
    watch_sender: watch::Sender<TorrentDownloadState>,
    watch_receiver: watch::Receiver<TorrentDownloadState>,
    torrent_download_state: TorrentDownloadState,
}

impl StatisticsManager {
    pub(crate) fn new(torrent_storage: &TorrentStorage) -> Self {
        let torrent_download_state = {
            let storage_state = torrent_storage.receiver.borrow();
            TorrentDownloadState {
                downloaded: storage_state.bytes_write,
                uploaded: storage_state.bytes_read,
            }
        };
        let (watch_sender, watch_receiver) = watch::channel(torrent_download_state.clone());
        Self {
            sender: None,
            watch_sender,
            watch_receiver,
            torrent_download_state,
        }
    }
}

#[async_trait]
impl EventLoopRunner<TorrentStatisticMessage, TorrentEvent> for StatisticsManager {
    fn set_sender(&mut self, sender: EventLoopSender<TorrentStatisticMessage, TorrentEvent>) {
        self.sender = Some(sender);
    }

    fn sender(&mut self) -> Option<&mut EventLoopSender<TorrentStatisticMessage, TorrentEvent>> {
        self.sender.as_mut()
    }

    async fn start(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn handle(&mut self, message: TorrentStatisticMessage) -> Result<(), RsbtError> {
        match message {
            TorrentStatisticMessage::Subscribe(request_response) => {
                if let Err(err) = request_response.response(self.watch_receiver.clone()) {
                    error!(
                        "cannot send subscription response to torrent statistics: {}",
                        err
                    );
                }
            }
            TorrentStatisticMessage::Uploaded(count) => {
                self.torrent_download_state.uploaded += count;
                if let Err(err) = self.watch_sender.broadcast(self.torrent_download_state) {
                    error!("cannot broadcast uploaded torrent statistics: {}", err);
                }
            }
            TorrentStatisticMessage::Downloaded(count) => {
                self.torrent_download_state.downloaded += count;
                if let Err(err) = self.watch_sender.broadcast(self.torrent_download_state) {
                    error!("cannot broadcast downloaded torrent statistics: {}", err);
                }
            }
        }
        Ok(())
    }
}
