use crate::{
    announce::AnnounceManagerMessage,
    event::TorrentEvent,
    event_loop::{EventLoopRunner, EventLoopSender},
    RsbtError,
};
use async_trait::async_trait;

#[derive(Default)]
pub(crate) struct AnnounceManager {
    sender: Option<EventLoopSender<AnnounceManagerMessage, TorrentEvent>>,
}

#[async_trait]
impl EventLoopRunner<AnnounceManagerMessage, TorrentEvent> for AnnounceManager {
    async fn start(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn quit(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    async fn handle(&mut self, _message: AnnounceManagerMessage) -> Result<(), RsbtError> {
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
