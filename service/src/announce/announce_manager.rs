use crate::{announce::AnnounceManagerMessage, event_loop::EventLoopRunner, RsbtError};

pub(crate) struct AnnounceManager {}

impl EventLoopRunner<AnnounceManagerMessage> for AnnounceManager {}

#[cfg(test)]
mod tests {}
