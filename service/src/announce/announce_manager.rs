use crate::{
    announce::{
        AnnounceManagerCommand, AnnounceManagerMessage, AnnounceManagerState, AnnounceTransport,
        Announcement, DefaultAnnounceTransport,
    },
    event::TorrentEvent,
    event_loop::{EventLoopCommand, EventLoopRunner, EventLoopSender},
    process::TorrentToken,
    types::{Properties, Torrent},
    RsbtError,
};
use async_trait::async_trait;
use log::{debug, error, warn};
use rand::{seq::SliceRandom, thread_rng};
use std::sync::Arc;
use tokio::time::{delay_for, Duration};
use Clone;

pub(crate) struct AnnounceManager<T: AnnounceTransport = DefaultAnnounceTransport> {
    announce_urls: Vec<Vec<String>>,
    sender: Option<EventLoopSender<AnnounceManagerMessage, TorrentEvent>>,
    state: AnnounceManagerState,
    transport: T,
}

impl<T: AnnounceTransport> AnnounceManager<T> {
    pub(crate) fn new(properties: Arc<Properties>, torrent_token: Arc<TorrentToken>) -> Self {
        let announce_urls = Self::shuffle_announce_urls(&torrent_token.torrent);
        Self {
            announce_urls,
            sender: None,
            state: AnnounceManagerState::Idle,
            transport: T::new(properties, torrent_token),
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

    async fn query_announce(
        &mut self,
        tier: usize,
        tracker: usize,
        delay: Option<Duration>,
    ) -> Result<(), RsbtError> {
        let command = self.command(
            Self::query_announce_command(
                self.announce_urls[tier][tracker].clone(),
                self.transport.clone(),
                tier,
                tracker,
                delay,
            ),
            AnnounceManagerMessage::QueryAnnounceResult,
        );
        self.set_running_state(AnnounceManagerCommand::Query { tier, tracker }, command);
        Ok(())
    }

    fn set_running_state(
        &mut self,
        parameters: AnnounceManagerCommand,
        command: Option<EventLoopCommand>,
    ) {
        if let Some(command) = command {
            self.state = AnnounceManagerState::Running {
                parameters,
                command,
            };
        }
    }

    async fn query_announce_command(
        url: String,
        transport: T,
        tier: usize,
        tracker: usize,
        delay: Option<Duration>,
    ) -> Result<Announcement, RsbtError> {
        if let Some(delay) = delay {
            debug!("await {:?} to requery announce...", delay);
            delay_for(delay).await;
        }
        debug!("query announce for tier {} tracker {}", tier, tracker);

        transport.request_announce(url).await
    }

    async fn query_announce_result(
        &mut self,
        result: Result<Announcement, RsbtError>,
    ) -> Result<(), RsbtError> {
        match &self.state {
            &AnnounceManagerState::Running {
                parameters: AnnounceManagerCommand::Query { tier, tracker },
                ..
            } => {
                self.process_announce(tier, tracker, result).await?;
            }
            _ => error!("cannot handle query announce result: wrong state"),
        }
        Ok(())
    }

    async fn process_announce(
        &mut self,
        tier: usize,
        tracker: usize,
        result: Result<Announcement, RsbtError>,
    ) -> Result<(), RsbtError> {
        debug!("process announce for tier {} tracker {}", tier, tracker);

        match result {
            Ok(announce) => {
                self.process_announce_ok(tier, tracker, announce).await?;
            }
            Err(err) => {
                self.process_announce_err(tier, tracker, err).await?;
            }
        }

        Ok(())
    }

    async fn process_announce_ok(
        &mut self,
        tier: usize,
        tracker: usize,
        announce: Announcement,
    ) -> Result<(), RsbtError> {
        if tracker != 0 {
            let tier = &mut self.announce_urls[tier];
            let tracker = tier.remove(tracker);
            tier.insert(0, tracker);
        }

        self.feedback(TorrentEvent::Announce(announce.peers))
            .await?;

        self.delayed_query_announce(announce.requery_interval).await
    }

    async fn default_query_announce(&mut self) -> Result<(), RsbtError> {
        self.delayed_query_announce(Duration::from_secs(60)).await
    }

    async fn delayed_query_announce(&mut self, delay: Duration) -> Result<(), RsbtError> {
        self.send_query_announce(0, 0, Some(delay)).await
    }

    async fn process_announce_err(
        &mut self,
        tier: usize,
        tracker: usize,
        err: RsbtError,
    ) -> Result<(), RsbtError> {
        error!("announce failure: {}", err);

        let tier_ref = &self.announce_urls[tier];

        if tier_ref.len() == tracker + 1 {
            if self.announce_urls.len() == tier + 1 {
                debug!("all urls failed, waiting before retry...");
                self.default_query_announce().await
            } else {
                self.send_query_announce(tier + 1, 0, None).await
            }
        } else {
            self.send_query_announce(tier, tracker + 1, None).await
        }
    }

    async fn send_query_announce(
        &mut self,
        tier: usize,
        tracker: usize,
        delay: Option<Duration>,
    ) -> Result<(), RsbtError> {
        self.send(AnnounceManagerMessage::QueryAnnounce {
            tier,
            tracker,
            delay,
        })
        .await
    }
}

#[async_trait]
impl<T: AnnounceTransport> EventLoopRunner<AnnounceManagerMessage, TorrentEvent>
    for AnnounceManager<T>
{
    async fn start(&mut self) -> Result<(), RsbtError> {
        match &self.state {
            AnnounceManagerState::Idle => {
                self.send_query_announce(0, 0, None).await?;
            }
            AnnounceManagerState::Running { .. } => {
                warn!("must be idle to start");
            }
        }
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), RsbtError> {
        match &self.state {
            AnnounceManagerState::Idle => {
                warn!("already stopped");
            }
            AnnounceManagerState::Running { command, .. } => {
                debug!("pending query announce, aborting...");
                command.abort();
            }
        }
        self.state = AnnounceManagerState::Idle;
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), RsbtError> {
        if !matches!(self.state, AnnounceManagerState::Idle) {
            self.stop().await?;
        }

        Ok(())
    }

    async fn handle(&mut self, message: AnnounceManagerMessage) -> Result<(), RsbtError> {
        match message {
            AnnounceManagerMessage::QueryAnnounce {
                tier,
                tracker,
                delay,
            } => self.query_announce(tier, tracker, delay).await?,
            AnnounceManagerMessage::QueryAnnounceResult(result) => {
                self.query_announce_result(result).await?
            }
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
    use super::*;

    #[derive(Clone, Default)]
    struct TestAnnounceTransport;

    #[tokio::test]
    async fn test() {}
}
