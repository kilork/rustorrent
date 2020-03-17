#[macro_use]
extern crate actix_web;

#[cfg(feature = "ui")]
use rsbt_web_resources::*;

use actix::prelude::*;
use actix_identity::{CookieIdentityPolicy, Identity, IdentityService};
use actix_multipart::Multipart;
use actix_service::Service;
use actix_web::{
    dev::Payload, error::ErrorUnauthorized, http, middleware, web, App, Error, FromRequest,
    HttpRequest, HttpResponse, HttpServer, Responder,
};
use bytes::Bytes;
use dotenv::dotenv;
use exitfailure::ExitFailure;
use futures::StreamExt;
use log::info;
use log::{debug, error};
use openid::{DiscoveredClient, Options, Token, Userinfo};
use reqwest;
use rsbt_service::{
    app::{RequestResponse, RsbtApp, RsbtCommand},
    types::Settings,
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::HashMap,
    pin::Pin,
    task::{Context, Poll},
};
use structopt::StructOpt;
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot, Mutex, RwLock,
    },
    time::{interval_at, Duration, Instant},
};
use url::Url;

mod cli;
mod event_stream;
mod login;
mod model;
mod torrents;
mod uploads;

use event_stream::*;
use login::*;
use model::*;
use torrents::*;
use uploads::*;

lazy_static::lazy_static! {
static ref RSBT_UI_HOST: String = std::env::var("RSBT_UI_HOST").unwrap_or_else(|_| "http://localhost:8080".to_string());
static ref RSBT_BIND: String = std::env::var("RSBT_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
static ref RSBT_OPENID_CLIENT_ID: String = std::env::var("RSBT_OPENID_CLIENT_ID").unwrap_or_else(|_| "web_app".to_string());
static ref RSBT_OPENID_CLIENT_SECRET: String = std::env::var("RSBT_OPENID_CLIENT_SECRET").unwrap_or_else(|_| "web_app".to_string());
static ref RSBT_OPENID_ISSUER: String = std::env::var("RSBT_OPENID_ISSUER").unwrap_or_else(|_| "http://keycloak:9080/auth/realms/jhipster".to_string());
static ref RSBT_ALLOW: String = std::env::var("RSBT_ALLOW").unwrap_or_else(|_| "user@localhost".to_string());
}

struct Sessions {
    map: HashMap<String, (User, Token, Userinfo)>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Failure {
    error: String,
}

fn host(path: &str) -> String {
    RSBT_UI_HOST.clone() + path
}

#[actix_rt::main]
async fn main() -> Result<(), ExitFailure> {
    dotenv().ok();

    let cli = cli::Cli::from_args();

    env_logger::init();

    let client_id = RSBT_OPENID_CLIENT_ID.to_string();
    let client_secret = RSBT_OPENID_CLIENT_SECRET.to_string();
    let redirect = Some(host("/login/oauth2/code/oidc"));
    let issuer = reqwest::Url::parse(RSBT_OPENID_ISSUER.as_str())?;
    debug!("redirect: {:?}", redirect);
    debug!("issuer: {}", issuer);
    let client = openid::Client::discover(client_id, client_secret, redirect, issuer).await?;

    debug!("discovered config: {:?}", client.config());

    let client = web::Data::new(client);

    let settings = Settings::default().override_with(cli.config);

    debug!("starting torrents process with settings: {:?}", settings);

    let rsbt_app = web::Data::new(RsbtApp::new(settings));
    let broadcaster = web::Data::new(RwLock::new(Broadcaster::new()));

    let sessions = web::Data::new(RwLock::new(Sessions {
        map: HashMap::new(),
    }));

    let broadcaster_timer = broadcaster.clone();

    let task = async move {
        let mut timer = interval_at(Instant::now(), Duration::from_secs(10));

        loop {
            timer.tick().await;

            debug!("timer event");

            let mut me = broadcaster_timer.write().await;

            if let Err(ok_clients) = me.message("ping") {
                debug!("refresh client list");
                me.clients = ok_clients;
            }
        }
    };
    Arbiter::spawn(task);

    let (download_events_sender, download_events_receiver) =
        mpsc::channel(rsbt_service::DEFAULT_CHANNEL_BUFFER);

    let rsbt_app_clone = rsbt_app.clone();
    let download_events_task_sender = download_events_sender.clone();
    let rsbt_app_task = async move {
        if let Err(err) = rsbt_app_clone
            .processing_loop(download_events_task_sender, download_events_receiver)
            .await
        {
            error!("problem detected: {}", err);
        }
    };
    Arbiter::spawn(rsbt_app_task);
    let sender = web::Data::new(Mutex::new(download_events_sender));

    HttpServer::new(move || {
        let mut app = App::new()
            .wrap(middleware::Logger::default())
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(&[0; 32])
                    .name("auth-rsbt")
                    .secure(false),
            ))
            .app_data(broadcaster.clone())
            .app_data(client.clone())
            .app_data(sessions.clone())
            .app_data(rsbt_app.clone())
            .app_data(sender.clone())
            .service(authorize)
            .service(login_get)
            .service(
                web::scope("/api")
                    .wrap_fn(|req, srv| {
                        let fut = srv.call(req);
                        async {
                            let res = fut.await?;
                            Ok(res)
                        }
                    })
                    .service(torrent_list)
                    .service(upload_form)
                    .service(upload)
                    .service(account)
                    .service(logout)
                    .service(stream),
            );
        #[cfg(feature = "ui")]
        {
            debug!("serving frontend files...");
            let generated_files = generate_files();
            app = app.service(actix_web_static_files::ResourceFiles::new(
                "/",
                generated_files,
            ));
        }
        app
    })
    .bind(RSBT_BIND.as_str())?
    .run()
    .await?;

    Ok(())
}
