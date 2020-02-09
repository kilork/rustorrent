#[macro_use]
extern crate actix_web;

use actix::prelude::*;

use actix_web::{web, App, Error, HttpResponse, HttpServer, Responder};
use actix_web_static_files;
use bytes::Bytes;
use exitfailure::ExitFailure;
use rustorrent_web_resources::*;
use std::{
    pin::Pin,
    sync::RwLock,
    task::{Context, Poll},
};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::{interval_at, Duration, Instant};

#[get("/torrents")]
async fn torrent_list() -> impl Responder {
    web::Json("")
}

#[get("/stream")]
async fn stream(broadcaster: web::Data<RwLock<Broadcaster>>) -> impl Responder {
    let rx = broadcaster.write().unwrap().new_client();
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .keep_alive()
        .no_chunking()
        .streaming(rx)
}

#[actix_rt::main]
async fn main() -> Result<(), ExitFailure> {
    let broadcaster = web::Data::new(RwLock::new(Broadcaster::new()));

    let broadcaster_timer = broadcaster.clone();
    let task = async move {
        let mut timer = interval_at(Instant::now(), Duration::from_secs(10));
        loop {
            timer.tick().await;
            eprintln!("timer event");
            let mut me = broadcaster_timer.write().unwrap();
            if let Err(ok_clients) = me.message("ping") {
                eprintln!("refresh client list");
                me.clients = ok_clients;
            }
        }
    };
    Arbiter::spawn(task);

    HttpServer::new(move || {
        let generated_files = generate_files();
        let generated_css = generate_css();
        App::new()
            .app_data(broadcaster.clone())
            .service(stream)
            .service(torrent_list)
            .service(actix_web_static_files::ResourceFiles::new(
                "/",
                generated_files,
            ))
            .service(actix_web_static_files::ResourceFiles::new(
                "/css",
                generated_css,
            ))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await?;

    Ok(())
}

struct Broadcaster {
    clients: Vec<Sender<Bytes>>,
}

impl Broadcaster {
    fn new() -> Self {
        Self { clients: vec![] }
    }

    fn new_client(&mut self) -> Client {
        eprintln!("adding new client");
        let (tx, rx) = channel(100);

        tx.clone()
            .try_send(Bytes::from("data: connected\n\n"))
            .unwrap();

        self.clients.push(tx);

        Client(rx)
    }

    fn message(&mut self, msg: &str) -> Result<(), Vec<Sender<Bytes>>> {
        let mut ok_clients = vec![];
        eprintln!("message to {} client(s)", self.clients.len());
        let msg = Bytes::from(["data: ", msg, "\n\n"].concat());
        for client in &mut self.clients {
            if let Ok(()) = client.try_send(msg.clone()) {
                ok_clients.push(client.clone())
            }
        }
        if ok_clients.len() != self.clients.len() {
            return Err(ok_clients);
        }
        Ok(())
    }
}

// wrap Receiver in own type, with correct error type
struct Client(Receiver<Bytes>);

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
