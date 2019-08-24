#[macro_use]
extern crate actix_web;

#[macro_use]
extern crate serde_json;

use actix_web::{web, App, Error, HttpResponse, HttpServer};
use actix_web_static_files;
use bytes::Bytes;
use exitfailure::ExitFailure;
use failure::ResultExt;
use futures::{Async, Poll, Stream};
use rustorrent_web_resources::*;
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tokio::timer::Interval;

#[derive(Serialize, Deserialize)]
struct TorrentInfo {
    name: String,
    len: usize,
}
struct AppState {
    torrents: RwLock<Vec<TorrentInfo>>,
}

const INDEX: &str = include_str!("../static/templates/index.html");

#[get("/")]
fn index() -> HttpResponse {
    HttpResponse::Ok().body(INDEX)
}

#[get("/stream")]
fn stream() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .no_chunking()
        .force_close()
        .streaming(Sse {
            interval: Interval::new(Instant::now(), Duration::from_millis(5000)),
        })
}

struct Sse {
    interval: Interval,
}

impl Stream for Sse {
    type Item = Bytes;
    type Error = Error;

    fn poll(&mut self) -> Poll<Option<Bytes>, Error> {
        match self.interval.poll() {
            Ok(Async::Ready(_)) => Ok(Async::Ready(Some(Bytes::from(&b"data: ping\n\n"[..])))),
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(_) => Ok(Async::Ready(None)),
        }
    }
}

fn main() -> Result<(), ExitFailure> {
    let app_state = web::Data::new(AppState {
        torrents: RwLock::new(vec![TorrentInfo {
            name: "ferris2.gif".into(),
            len: 308189,
        }]),
    });

    HttpServer::new(move || {
        let generated_files = generate_files();
        let generated_css = generate_css();
        App::new()
            .register_data(app_state.clone())
            .service(index)
            .service(stream)
            .service(actix_web_static_files::ResourceFiles::new(
                "/files",
                generated_files,
            ))
            .service(actix_web_static_files::ResourceFiles::new(
                "/css",
                generated_css,
            ))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .map_err(|x| x.into())
}
