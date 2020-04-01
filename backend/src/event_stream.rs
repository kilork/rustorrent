use super::*;

#[get("/stream")]
async fn stream(broadcaster: web::Data<RwLock<Broadcaster>>, _user: User) -> impl Responder {
    let rx = broadcaster.write().await.new_client().await;
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .keep_alive()
        .no_chunking()
        .streaming(rx)
}

pub(crate) struct Broadcaster {
    pub(crate) clients: RwLock<Vec<Sender<Bytes>>>,
}

impl Broadcaster {
    pub(crate) fn new() -> Self {
        Self {
            clients: RwLock::new(vec![]),
        }
    }

    pub(crate) async fn new_client(&mut self) -> Client {
        eprintln!("adding new client");
        let (tx, rx) = mpsc::channel(100);

        tx.clone()
            .try_send(Bytes::from("data: connected\n\n"))
            .unwrap();

        self.clients.write().await.push(tx);

        Client(rx)
    }

    pub(crate) async fn message(&self, msg: &str) -> Result<(), Vec<Sender<Bytes>>> {
        let mut ok_clients = vec![];

        let len = self.clients.read().await.len();
        debug!("message to {} client(s)", len);
        let msg = Bytes::from(["data: ", msg, "\n\n"].concat());

        for client in self.clients.write().await.iter_mut() {
            if let Ok(()) = client.try_send(msg.clone()) {
                ok_clients.push(client.clone())
            }
        }

        if ok_clients.len() != len {
            return Err(ok_clients);
        }

        Ok(())
    }
}

// wrap Receiver in own type, with correct error type
pub(crate) struct Client(Receiver<Bytes>);

impl Stream for Client {
    type Item = Result<Bytes, Error>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match Pin::new(&mut self.0).poll_next(cx) {
            Poll::Ready(Some(v)) => Poll::Ready(Some(Ok(v))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
