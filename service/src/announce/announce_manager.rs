use crate::{event_loop_runner::EventLoopRunner, RsbtError};

pub(crate) struct AnnounceManager {}

impl AnnounceManager {
    pub(crate) async fn enable(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }

    pub(crate) async fn disable(&mut self) -> Result<(), RsbtError> {
        Ok(())
    }
}

impl EventLoopRunner for AnnounceManager {}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test() {}
}
