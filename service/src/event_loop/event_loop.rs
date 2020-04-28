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

pub(crate) struct EventLoop<M, T, F> {
    join_handle: Option<JoinHandle<T>>,
    sender: mpsc::Sender<EventLoopMessage<M, F>>,
    feedback: mpsc::Sender<F>,
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

        let mut event_loop_sender = sender.clone().into();
        let mut feedback_loop_sender = feedback.clone();

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
                        debug!("loop");
                        if let Err(err) = runner.handle(message, &mut event_loop_sender).await {
                            error!("runner cannot handle message: {}", err);
                        }
                    }
                    EventLoopMessage::Feedback(message) => {
                        debug!("feedback");
                        if let Err(err) = feedback_loop_sender.send(message).await {
                            error!("cannot forward feedback message: {}", err);
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
            feedback,
        })
    }

    pub(crate) async fn send<E: Into<EventLoopMessage<M, F>>>(
        &mut self,
        message: E,
    ) -> Result<(), RsbtError> {
        self.sender.send(message.into()).await?;

        Ok(())
    }

    async fn request<R, FN>(&mut self, message_fn: FN) -> Result<R, RsbtError>
    where
        FN: Fn(oneshot::Sender<Result<R, RsbtError>>) -> EventLoopMessage<M, F>,
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

    #[derive(Debug, PartialEq, Default)]
    struct TestLoop {
        message_count: usize,
        retransfer_count: usize,
        feedback_count: usize,
    }

    #[async_trait]
    impl EventLoopRunner<TestMessage, TestFeedbackMessage> for TestLoop {
        async fn handle(
            &mut self,
            message: TestMessage,
            event_loop_sender: &mut EventLoopSender<TestMessage, TestFeedbackMessage>,
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
                TestMessage::TestFeedBack(test_data) => {
                    self.feedback_count += 1;
                    event_loop_sender
                        .feedback(TestFeedbackMessage::Feedback(test_data))
                        .await?;
                }
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_loop_quit() {
        let (feedback_sender, receiver) = mpsc::channel(1);
        let mut handler: EventLoop<TestMessage, TestLoop, TestFeedbackMessage> =
            EventLoop::spawn(Default::default(), feedback_sender).expect("cannot spawn test loop");
        let test_loop = handler.quit().await.expect("cannot quit test loop");
        assert_eq!(test_loop, Some(Default::default()));
    }

    #[tokio::test]
    async fn test_loop_retransfer() {
        let (feedback_sender, receiver) = mpsc::channel(1);
        let mut handler: EventLoop<TestMessage, TestLoop, TestFeedbackMessage> =
            EventLoop::spawn(Default::default(), feedback_sender).expect("cannot spawn test loop");
        handler
            .send(TestMessage::TestData(vec![1, 2, 3, 4]))
            .await
            .expect("cannot send test message");
        let test_loop = handler.wait().await.expect("cannot quit test loop");
        assert_eq!(
            test_loop,
            Some(TestLoop {
                message_count: 1,
                retransfer_count: 1,
                feedback_count: 0,
            })
        );
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
        assert_eq!(
            test_loop,
            Some(TestLoop {
                message_count: 0,
                retransfer_count: 0,
                feedback_count: 1,
            })
        );
    }
}
