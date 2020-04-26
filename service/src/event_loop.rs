use crate::{
    event_loop_message::EventLoopMessage, event_loop_runner::EventLoopRunner, RsbtError,
    DEFAULT_CHANNEL_BUFFER,
};
use log::error;
use tokio::{
    stream::StreamExt,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

pub(crate) struct EventLoop<M> {
    join_handle: Option<JoinHandle<()>>,
    sender: mpsc::Sender<EventLoopMessage<M>>,
}

impl<M: Send + 'static> EventLoop<M> {
    pub(crate) fn spawn<T>(mut runner: T) -> Result<EventLoop<M>, RsbtError>
    where
        T: Send + EventLoopRunner + 'static,
    {
        let (sender, mut receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let join_handle = Some(tokio::spawn(async move {
            while let Some(event) = receiver.next().await {
                match event {
                    EventLoopMessage::Start(sender) => {
                        if let Err(_) = sender.send(runner.start().await) {
                            error!("cannot respond after start runner");
                        }
                    }
                    EventLoopMessage::Stop(sender) => {
                        if let Err(_) = sender.send(runner.stop().await) {
                            error!("cannot respond after stop runner");
                        }
                    }
                    EventLoopMessage::Quit(sender) => {
                        let quit_result = runner.quit().await;
                        let we_done = quit_result.is_ok();
                        if let Err(_) = sender.send(quit_result) {
                            error!("cannot respond after quit runner");
                        }
                        if we_done {
                            break;
                        }
                    }
                    EventLoopMessage::Loop(_) => {}
                }
            }
        }));

        Ok(EventLoop {
            join_handle,
            sender,
        })
    }

    pub(crate) async fn send(&mut self, message: M) -> Result<(), RsbtError> {
        self.sender.send(message.into()).await?;

        Ok(())
    }

    async fn request<R, F>(&mut self, message_fn: F) -> Result<R, RsbtError>
    where
        F: Fn(oneshot::Sender<Result<R, RsbtError>>) -> EventLoopMessage<M>,
    {
        let (sender, receiver) = oneshot::channel();

        self.sender.send(message_fn(sender)).await?;

        receiver.await?
    }

    pub(crate) async fn start(&mut self) -> Result<(), RsbtError> {
        self.request(EventLoopMessage::Start).await
    }

    pub(crate) async fn stop(&mut self) -> Result<(), RsbtError> {
        self.request(EventLoopMessage::Stop).await
    }

    pub(crate) async fn quit(&mut self) -> Result<(), RsbtError> {
        self.request(EventLoopMessage::Quit).await?;

        if let Some(join_handle) = self.join_handle.take() {
            join_handle.await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::{EventLoop, EventLoopRunner, RsbtError};

    enum TestMessage {
        TestData(Vec<u8>),
    }

    struct TestLoop {}

    impl EventLoopRunner for TestLoop {}

    #[tokio::test]
    async fn test_loop() {
        let mut handler: EventLoop<TestMessage> =
            EventLoop::spawn(TestLoop {}).expect("cannot spawn test loop");
        handler.quit().await.expect("cannot quit test loop");
    }
}
