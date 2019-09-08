#[macro_use]
extern crate actix_web;

#[macro_use]
extern crate serde_json;

use actix::prelude::*;
use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use actix_web_static_files;
use bytes::Bytes;
use exitfailure::ExitFailure;
use failure::{Error, ResultExt};
use futures::{Async, Poll, Stream};
use rustorrent_web_resources::*;
use serde::{Deserialize, Serialize};
use std::io;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use tokio::timer::Interval;

#[derive(Serialize, Deserialize, Clone)]
struct TorrentInfo {
    name: String,
    total: usize,
    downloaded: usize,
    uploaded: usize,
    active: bool,
}

struct AppState {
    torrents: Vec<TorrentInfo>,
}

const INDEX: &str = include_str!("../static/templates/index.html");

#[get("/")]
fn index() -> impl Responder {
    HttpResponse::Ok().body(INDEX)
}

#[get("/torrents")]
fn torrent_list(app_state: web::Data<AppState>) -> impl Responder {
    web::Json(app_state.torrents.clone())
}

#[get("/stream")]
fn stream() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .keep_alive()
        .streaming(
            Interval::new(Instant::now(), Duration::from_millis(5000))
                .map(|_| Bytes::from(&b"data: ping\n\n"[..]))
                .map_err(|_| ()),
        )
}

fn main() -> Result<(), ExitFailure> {
    let system = System::new(env!("CARGO_PKG_NAME"));
    let app_state = web::Data::new(AppState {
        torrents: vec![TorrentInfo {
            name: "ferris2.gif".into(),
            total: 308_189,
            downloaded: 100_100,
            uploaded: 55_020,
            active: true,
        }],
    });

    HttpServer::new(move || {
        let generated_files = generate_files();
        let generated_css = generate_css();
        App::new()
            .register_data(app_state.clone())
            .service(index)
            .service(stream)
            .service(torrent_list)
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
    .start();

    system.run().map_err(|x| x.into())
}
