use crate::{
    event_loop::{EventLoopMessage, EventLoopRunner},
    RsbtError, DEFAULT_CHANNEL_BUFFER,
};
use log::{debug, error};
use std::clone::Clone;
use tokio::{
    stream::StreamExt,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

pub(crate) struct EventLoop<M, T> {
    join_handle: Option<JoinHandle<T>>,
    sender: mpsc::Sender<EventLoopMessage<M>>,
}

impl<M: Send + 'static, T> EventLoop<M, T> {
    pub(crate) fn spawn(mut runner: T) -> Result<EventLoop<M, T>, RsbtError>
    where
        T: Send + EventLoopRunner<M> + 'static,
    {
        let (sender, mut receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let mut event_loop_sender = sender.clone().into();

        let join_handle = Some(tokio::spawn(async move {
            while let Some(event) = receiver.next().await {
                match event {
                    EventLoopMessage::Start(sender) => {
                        debug!("start");
                        if let Err(_) = sender.send(runner.start().await) {
                            error!("cannot respond after start runner");
                        }
                    }
                    EventLoopMessage::Stop(sender) => {
                        debug!("stop");
                        if let Err(_) = sender.send(runner.stop().await) {
                            error!("cannot respond after stop runner");
                        }
                    }
                    EventLoopMessage::Quit(sender) => {
                        debug!("quit");
                        let quit_result = runner.quit().await;
                        let we_done = quit_result.is_ok();
                        if let Some(sender) = sender {
                            if let Err(_) = sender.send(quit_result) {
                                error!("cannot respond after quit runner");
                            }
                        }
                        if we_done {
                            break;
                        }
                    }
                    EventLoopMessage::Loop(message) => {
                        if let Err(err) = runner.handle(message, &mut event_loop_sender).await {
                            error!("runner cannot handle message: {}", err);
                        }
                    }
                }
            }
            debug!("loop done");
            runner
        }));

        Ok(EventLoop {
            join_handle,
            sender,
        })
    }

    pub(crate) async fn send<E: Into<EventLoopMessage<M>>>(
        &mut self,
        message: E,
    ) -> Result<(), RsbtError> {
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

    pub(crate) async fn quit(&mut self) -> Result<Option<T>, RsbtError> {
        self.request(|sender| EventLoopMessage::Quit(Some(sender)))
            .await?;

        self.wait().await
    }

    pub(crate) async fn wait(&mut self) -> Result<Option<T>, RsbtError> {
        Ok(if let Some(join_handle) = self.join_handle.take() {
            Some(join_handle.await?)
        } else {
            None
        })
    }
}

#[cfg(test)]
mod tests {

    use super::{EventLoop, EventLoopRunner, RsbtError};
    use crate::event_loop::{EventLoopMessage, EventLoopSender};
    use async_trait::async_trait;

    enum TestMessage {
        TestData(Vec<u8>),
        TestRetransfer(Vec<u8>),
    }

    #[derive(Debug, PartialEq)]
    struct TestLoop {
        message_count: usize,
        retransfer_count: usize,
    }

    #[async_trait]
    impl EventLoopRunner<TestMessage> for TestLoop {
        async fn handle(
            &mut self,
            message: TestMessage,
            event_loop_sender: &mut EventLoopSender<TestMessage>,
        ) -> Result<(), RsbtError> {
            match message {
                TestMessage::TestData(data) => {
                    self.message_count += 1;
                    event_loop_sender
                        .send(TestMessage::TestRetransfer(data))
                        .await?;
                }
                TestMessage::TestRetransfer(_) => {
                    self.retransfer_count += 1;
                    event_loop_sender.send(EventLoopMessage::Quit(None)).await?;
                }
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_loop_quit() {
        let mut handler: EventLoop<TestMessage, _> = EventLoop::spawn(TestLoop {
            message_count: 0,
            retransfer_count: 0,
        })
        .expect("cannot spawn test loop");
        let test_loop = handler.quit().await.expect("cannot quit test loop");
        assert_eq!(
            test_loop,
            Some(TestLoop {
                message_count: 0,
                retransfer_count: 0
            })
        );
    }

    #[tokio::test]
    async fn test_loop_retransfer() {
        let mut handler: EventLoop<TestMessage, _> = EventLoop::spawn(TestLoop {
            message_count: 0,
            retransfer_count: 0,
        })
        .expect("cannot spawn test loop");
        handler
            .send(TestMessage::TestData(vec![1, 2, 3, 4]))
            .await
            .expect("cannot send test message");
        let test_loop = handler.wait().await.expect("cannot quit test loop");
        assert_eq!(
            test_loop,
            Some(TestLoop {
                message_count: 1,
                retransfer_count: 1
            })
        );
    }
}
