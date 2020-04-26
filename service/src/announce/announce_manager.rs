use crate::{event_loop_runner::EventLoopRunner, RsbtError};

pub(crate) struct AnnounceManager {}

impl EventLoopRunner for AnnounceManager {}

#[cfg(test)]
mod tests {}
