use crate::{
    event_loop::{EventLoopMessage, EventLoopRunner, EventLoopSender},
    RsbtError, DEFAULT_CHANNEL_BUFFER,
};
use log::{debug, error};
use std::clone::Clone;
use tokio::{
    stream::StreamExt,
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

pub(crate) struct EventLoop<M, T, F> {
    join_handle: Option<JoinHandle<T>>,
    loop_sender: EventLoopSender<M, F>,
    sender: mpsc::Sender<EventLoopMessage<M>>,
}

impl<M: Send + 'static, T, F: Send + 'static> EventLoop<M, T, F> {
    pub(crate) fn spawn(
        mut runner: T,
        feedback: mpsc::Sender<F>,
    ) -> Result<EventLoop<M, T, F>, RsbtError>
    where
        T: Send + EventLoopRunner<M, F> + 'static,
    {
        let (sender, mut receiver) = mpsc::channel(DEFAULT_CHANNEL_BUFFER);

        let loop_sender = EventLoopSender::new(sender.clone(), feedback.clone());
        runner.set_sender(loop_sender.clone());

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
                        if let Err(err) = runner.handle(message).await {
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
            loop_sender,
            sender,
        })
    }

    pub(crate) fn loop_sender(&self) -> &EventLoopSender<M, F> {
        &self.loop_sender
    }

    pub(crate) async fn send<E: Into<EventLoopMessage<M>>>(
        &mut self,
        message: E,
    ) -> Result<(), RsbtError> {
        self.sender.send(message.into()).await?;

        Ok(())
    }

    async fn request<R, FN>(&mut self, message_fn: FN) -> Result<R, RsbtError>
    where
        FN: Fn(oneshot::Sender<Result<R, RsbtError>>) -> EventLoopMessage<M>,
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

    use super::{mpsc, EventLoop, EventLoopRunner, RsbtError};
    use crate::event_loop::{EventLoopMessage, EventLoopSender};
    use async_trait::async_trait;
    use tokio::stream::StreamExt;

    #[derive(Debug, PartialEq)]
    enum TestFeedbackMessage {
        Feedback(usize),
    }

    enum TestMessage {
        TestData(Vec<u8>),
        TestRetransfer(Vec<u8>),
        TestFeedBack(usize),
    }

    #[derive(Default)]
    struct TestLoop {
        message_count: usize,
        retransfer_count: usize,
        feedback_count: usize,
        sender: Option<EventLoopSender<TestMessage, TestFeedbackMessage>>,
    }

    #[async_trait]
    impl EventLoopRunner<TestMessage, TestFeedbackMessage> for TestLoop {
        async fn handle(&mut self, message: TestMessage) -> Result<(), RsbtError> {
            match message {
                TestMessage::TestData(data) => {
                    self.message_count += 1;
                    self.send(TestMessage::TestRetransfer(data)).await?;
                }
                TestMessage::TestRetransfer(_) => {
                    self.retransfer_count += 1;
                    self.send(EventLoopMessage::Quit(None)).await?;
                }
                TestMessage::TestFeedBack(test_data) => {
                    self.feedback_count += 1;
                    self.feedback(TestFeedbackMessage::Feedback(test_data))
                        .await?;
                }
            }
            Ok(())
        }
        fn set_sender(&mut self, sender: EventLoopSender<TestMessage, TestFeedbackMessage>) {
            self.sender = Some(sender);
        }
        fn sender(&mut self) -> Option<&mut EventLoopSender<TestMessage, TestFeedbackMessage>> {
            self.sender.as_mut()
        }
    }

    #[tokio::test]
    async fn test_loop_quit() {
        let (feedback_sender, _receiver) = mpsc::channel(1);
        let mut handler: EventLoop<TestMessage, TestLoop, TestFeedbackMessage> =
            EventLoop::spawn(Default::default(), feedback_sender).expect("cannot spawn test loop");
        let test_loop = handler.quit().await.expect("cannot quit test loop");
        assert!(matches!(
            test_loop,
            Some(TestLoop {
                message_count: 0,
                retransfer_count: 0,
                feedback_count: 0,
                sender: _,
            })
        ));
    }

    #[tokio::test]
    async fn test_loop_retransfer() {
        let (feedback_sender, _receiver) = mpsc::channel(1);
        let mut handler: EventLoop<TestMessage, TestLoop, TestFeedbackMessage> =
            EventLoop::spawn(Default::default(), feedback_sender).expect("cannot spawn test loop");
        handler
            .send(TestMessage::TestData(vec![1, 2, 3, 4]))
            .await
            .expect("cannot send test message");
        let test_loop = handler.wait().await.expect("cannot quit test loop");
        assert!(matches!(
            test_loop,
            Some(TestLoop {
                message_count: 1,
                retransfer_count: 1,
                feedback_count: 0,
                sender: _,
            })
        ));
    }
    #[tokio::test]
    async fn test_loop_feedback() {
        let (feedback_sender, mut receiver) = mpsc::channel(1);
        let mut handler: EventLoop<TestMessage, TestLoop, TestFeedbackMessage> =
            EventLoop::spawn(Default::default(), feedback_sender).expect("cannot spawn test loop");
        handler
            .send(TestMessage::TestFeedBack(100))
            .await
            .expect("cannot send test message");
        let feedback_message = receiver.next().await;
        assert_eq!(feedback_message, Some(TestFeedbackMessage::Feedback(100)));
        let test_loop = handler.quit().await.expect("cannot quit test loop");
        assert!(matches!(
            test_loop,
            Some(TestLoop {
                message_count: 0,
                retransfer_count: 0,
                feedback_count: 1,
                sender: _,
            })
        ));
    }
}
