#[macro_use]
extern crate actix_web;

#[macro_use]
extern crate serde_json;

use actix_web::{web, App, HttpResponse, HttpServer};
use actix_web_static_files;
use exitfailure::ExitFailure;
use failure::ResultExt;
use rustorrent_web_resources::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::sync::RwLock;

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
