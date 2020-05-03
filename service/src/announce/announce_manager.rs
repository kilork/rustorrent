use crate::{
    announce::AnnounceManagerMessage,
    event::TorrentEvent,
    event_loop::{EventLoopRunner, EventLoopSender},
    process::TorrentToken,
    types::{Properties, Torrent},
    RsbtError,
};
use async_trait::async_trait;
use rand::{seq::SliceRandom, thread_rng};
use std::sync::Arc;

pub(crate) struct AnnounceManager {
    announce_urls: Vec<Vec<String>>,
    sender: Option<EventLoopSender<AnnounceManagerMessage, TorrentEvent>>,
    properties: Arc<Properties>,
    torrent_token: Arc<TorrentToken>,
}

impl AnnounceManager {
    pub(crate) fn new(properties: Arc<Properties>, torrent_token: Arc<TorrentToken>) -> Self {
        let announce_urls = Self::shuffle_announce_urls(&torrent_token.torrent);
        Self {
            announce_urls,
            sender: None,
            properties,
            torrent_token,
        }
    }

    fn shuffle_announce_urls(torrent: &Torrent) -> Vec<Vec<String>> {
        torrent
            .announce_list
            .as_ref()
            .map(|x| {
                let mut copied = x.clone();
                copied.iter_mut().for_each(|x| x.shuffle(&mut thread_rng()));
                copied
            })
            .unwrap_or_else(|| vec![vec![torrent.announce_url.clone()]])
    }

    async fn query_announce(&mut self, tier: usize, tracker: usize) -> Result<(), RsbtError> {
        Ok(())
    }
}

#[async_trait]
impl EventLoopRunner<AnnounceManagerMessage, TorrentEvent> for AnnounceManager {
    async fn start(&mut self) -> Result<(), RsbtError> {
        self.send(AnnounceManagerMessage::QueryAnnounce {
            tier: 0,
            tracker: 0,
        })
        .await
    }

    async fn stop(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn handle(&mut self, message: AnnounceManagerMessage) -> Result<(), RsbtError> {
        match message {
            AnnounceManagerMessage::QueryAnnounce { tier, tracker } => {
                self.query_announce(tier, tracker).await?
            }
            _ => (),
        }
        Ok(())
    }

    fn set_sender(&mut self, sender: EventLoopSender<AnnounceManagerMessage, TorrentEvent>) {
        self.sender = Some(sender);
    }

    fn sender(&mut self) -> Option<&mut EventLoopSender<AnnounceManagerMessage, TorrentEvent>> {
        self.sender.as_mut()
    }
}

#[cfg(test)]
mod tests {

    //
}
