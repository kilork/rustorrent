use crate::{
    announce::AnnounceManagerMessage, event::TorrentEvent, event_loop::EventLoopRunner, RsbtError,
};

pub(crate) struct AnnounceManager {}

impl EventLoopRunner<AnnounceManagerMessage, TorrentEvent> for AnnounceManager {}

#[cfg(test)]
mod tests {

    //
}
