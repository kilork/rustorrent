use crate::{announce::AnnounceManagerCommand, event_loop::EventLoopCommand};

pub(crate) enum AnnounceManagerState {
    Idle,
    Running {
        parameters: AnnounceManagerCommand,
        command: EventLoopCommand,
    },
}
