#[macro_use]
extern crate actix_web;

use actix::prelude::*;

use actix_web::{web, App, Error, HttpResponse, HttpServer, Responder};
use actix_web_static_files;
use bytes::Bytes;
use exitfailure::ExitFailure;
use log::{debug, info};
use oidc;
use reqwest;
use rustorrent_web_resources::*;
use std::{
    pin::Pin,
    sync::{Mutex, RwLock},
    task::{Context, Poll},
};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task,
    time::{interval_at, Duration, Instant},
};

#[get("/torrents")]
async fn torrent_list() -> impl Responder {
    web::Json("")
}

#[get("/oauth2/authorization/oidc")]
async fn authorize() -> impl Responder {
    // 302 Location: http://keycloak:9080/auth/realms/jhipster/protocol/openid-connect/auth?response_type=code&client_id=web_app&scope=openid%20address%20email%20jhipster%20microprofile-jwt%20offline_access%20phone%20profile%20roles%20web-origins&state=EP8ZhX1y0SsEyARdX3HUROfkbk2G1xvtfhChN2ujdsU%3D&redirect_uri=http://localhost:8080/login/oauth2/code/oidc&nonce=zPWbNSrAfBM5rg_uHjQ5Sb5ESusQUlMndIglOvxKVt0
    // http://keycloak:9080/auth/realms/jhipster/protocol/openid-connect/auth?response_type=code&client_id=web_app&scope=openid%20address%20email%20jhipster%20microprofile-jwt%20offline_access%20phone%20profile%20roles%20web-origins&redirect_uri=http://localhost:8080/login/oauth2/code/oidc
    // http://localhost:8080/login/oauth2/code/oidc?state=EP8ZhX1y0SsEyARdX3HUROfkbk2G1xvtfhChN2ujdsU%3D&session_state=1c1e04a6-2a99-4018-b77e-2fc653a5e333&code=028070ae-cf3b-4934-a7a9-8c4b25e16247.1c1e04a6-2a99-4018-b77e-2fc653a5e333.1eabef67-6473-4ba8-b07c-14bdbae4aaed
    // 302 http://localhost:8080
    debug!("hello from authorize");
    web::Json("{}")
}

#[post("/login/oauth2/code/oidc")]
async fn login() -> impl Responder {
    debug!("hello from login");
    HttpResponse::Ok()
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
    env_logger::init();
    let client = task::spawn_blocking(move || {
        let client_id = "web_app".to_string();
        let client_secret = "web_app".to_string();
        let redirect = reqwest::Url::parse("http://localhost:8080/login/oauth2/code/oidc")?;
        let issuer = reqwest::Url::parse("http://keycloak:9080/auth/realms/jhipster")?;
        let client = oidc::Client::discover(client_id, client_secret, redirect, issuer)?;
        Ok::<oidc::Client, ExitFailure>(client)
    }).await??;

    let auth_url = client.auth_url(&Default::default());
    info!("auth: {}", auth_url);

    let broadcaster = web::Data::new(RwLock::new(Broadcaster::new()));

    let broadcaster_timer = broadcaster.clone();
    let task = async move {
        let mut timer = interval_at(Instant::now(), Duration::from_secs(10));
        loop {
            timer.tick().await;
            debug!("timer event");
            let mut me = broadcaster_timer.write().unwrap();
            if let Err(ok_clients) = me.message("ping") {
                debug!("refresh client list");
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
            .service(authorize)
            .service(login)
            .service(actix_web_static_files::ResourceFiles::new(
                "/css",
                generated_css,
            ))
            .service(actix_web_static_files::ResourceFiles::new(
                "/",
                generated_files,
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
        debug!("message to {} client(s)", self.clients.len());
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
