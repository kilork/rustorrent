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
use tokio::{
    sync::{
        mpsc::{self, Receiver, Sender},
        oneshot, Mutex, RwLock,
    },
    time::{interval_at, Duration, Instant},
};
use url::Url;

mod model;

use model::*;

lazy_static::lazy_static! {
static ref RSBT_UI_HOST: String = std::env::var("RSBT_UI_HOST").unwrap_or_else(|_| "http://localhost:8080".to_string());
static ref RSBT_BIND: String = std::env::var("RSBT_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
static ref RSBT_OPENID_CLIENT_ID: String = std::env::var("RSBT_OPENID_CLIENT_ID").unwrap_or_else(|_| "web_app".to_string());
static ref RSBT_OPENID_CLIENT_SECRET: String = std::env::var("RSBT_OPENID_CLIENT_SECRET").unwrap_or_else(|_| "web_app".to_string());
static ref RSBT_OPENID_ISSUER: String = std::env::var("RSBT_OPENID_ISSUER").unwrap_or_else(|_| "http://keycloak:9080/auth/realms/jhipster".to_string());
static ref RSBT_ALLOW: String = std::env::var("RSBT_ALLOW").unwrap_or_else(|_| "user@localhost".to_string());
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
struct User {
    id: String,
    login: Option<String>,
    first_name: Option<String>,
    last_name: Option<String>,
    email: Option<String>,
    image_url: Option<String>,
    activated: bool,
    lang_key: Option<String>,
    authorities: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Logout {
    id_token: String,
    logout_url: Option<Url>,
}

impl FromRequest for User {
    type Config = ();
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<User, Error>>>>;

    fn from_request(req: &HttpRequest, pl: &mut Payload) -> Self::Future {
        let fut = Identity::from_request(req, pl);
        let sessions: Option<&web::Data<RwLock<Sessions>>> = req.app_data();
        if sessions.is_none() {
            error!("sessions is none!");
            return Box::pin(async { Err(ErrorUnauthorized("unauthorized")) });
        }
        let sessions = sessions.unwrap().clone();

        Box::pin(async move {
            if let Some(identity) = fut.await?.identity() {
                if let Some(user) = sessions
                    .read()
                    .await
                    .map
                    .get(&identity)
                    .map(|x| x.0.clone())
                {
                    return Ok(user);
                }
            };

            Err(ErrorUnauthorized("unauthorized"))
        })
    }
}

struct Sessions {
    map: HashMap<String, (User, Token, Userinfo)>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Failure {
    error: String,
}

struct Paging {
    page: Option<usize>,
    size: Option<usize>,
    sort: Vec<String>,
}

impl FromRequest for Paging {
    type Config = ();
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Paging, Error>>>>;

    fn from_request(req: &HttpRequest, _pl: &mut Payload) -> Self::Future {
        let query_string = req.query_string().as_bytes().to_vec();

        Box::pin(async move {
            let mut page = None;
            let mut size = None;
            let mut sort = vec![];
            for (key, value) in url::form_urlencoded::parse(&query_string).into_owned() {
                match key.as_str() {
                    "page" => match value.parse() {
                        Ok(page_value) => page = Some(page_value),
                        Err(err) => {
                            return Err(actix_web::error::ErrorUnprocessableEntity(format!(
                                "{}",
                                err
                            )))
                        }
                    },
                    "size" => match value.parse() {
                        Ok(size_value) => size = Some(size_value),
                        Err(err) => {
                            return Err(actix_web::error::ErrorUnprocessableEntity(format!(
                                "{}",
                                err
                            )))
                        }
                    },
                    "sort" => {
                        sort.push(value);
                    }
                    other => {
                        return Err(actix_web::error::ErrorUnprocessableEntity(format!(
                            "Unexpected key: {}",
                            other
                        )))
                    }
                }
            }
            Ok(Paging { page, size, sort })
        })
    }
}

#[get("/torrents")]
async fn torrent_list(
    paging: Paging,
    event_sender: web::Data<Mutex<Sender<RsbtCommand>>>,
    _user: User,
) -> impl Responder {
    let (sender, receiver) = oneshot::channel();

    {
        let mut event_sender = event_sender.lock().await;
        if let Err(err) = event_sender.send(RsbtCommand::TorrentList { sender }).await {
            error!("cannot send to torrent process: {}", err);
            return HttpResponse::InternalServerError().json(Failure {
                error: format!("cannot send to torrent process: {}", err),
            });
        }
    }

    match receiver.await {
        Ok(torrents) => {
            let mut torrents: Vec<_> = torrents
                .iter()
                .map(|torrent| TorrentDownload {
                    id: torrent.id,
                    name: torrent.name.as_str().into(),
                    received: 0,
                    uploaded: 0,
                    length: torrent.process.info.length,
                    active: true,
                })
                .collect();
            {
                let mut fields_order: Box<dyn Fn(&TorrentDownload, &TorrentDownload) -> Ordering> =
                    Box::new(|_, _| Ordering::Equal);

                let mut sorted_fields = paging
                    .sort
                    .iter()
                    .map(|x| x.split(','))
                    .map(|mut x| (x.next(), x.next()));
                while let Some((Some(field), order)) = sorted_fields.next() {
                    info!("order by field {} {:?}", field, order);

                    let mut field_order: Box<
                        dyn Fn(&TorrentDownload, &TorrentDownload) -> Ordering,
                    > = match field {
                        "id" => Box::new(|a, b| a.id.cmp(&b.id)),
                        "name" => Box::new(|a, b| a.name.cmp(&b.name)),
                        _ => panic!(),
                    };

                    let descending = order == Some("desc");
                    if descending {
                        field_order = Box::new(move |a, b| field_order(a, b).reverse());
                    }

                    fields_order = Box::new(move |a, b| fields_order(a, b).then(field_order(a, b)));
                }

                torrents.sort_by(|a, b| fields_order(a, b));
            }

            let page = paging.page.unwrap_or_default();
            let size = paging.size.unwrap_or(20);

            HttpResponse::Ok()
                .json::<Vec<_>>(torrents.iter().skip(page * size).take(size).collect())
        }
        Err(err) => {
            error!("error in receiver: {}", err);
            HttpResponse::InternalServerError().json(Failure {
                error: format!("cannot receive from torrent process: {}", err),
            })
        }
    }
}

#[get("/upload")]
async fn upload_form(_user: User) -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../static/upload.html"))
}

#[post("/upload")]
async fn upload(
    _user: User,
    event_sender: web::Data<Mutex<Sender<RsbtCommand>>>,
    mut payload: Multipart,
) -> Result<HttpResponse, Error> {
    while let Some(item) = payload.next().await {
        let mut field = item?;
        let content_type = field.content_disposition().unwrap();
        let filename = content_type.get_filename().unwrap();

        let mut torrent = vec![];
        while let Some(chunk) = field.next().await {
            let data = chunk?;
            torrent.extend(&data);
        }

        let mut event_sender = event_sender.lock().await;
        if let Err(err) = event_sender
            .send(RsbtCommand::AddTorrent(
                RequestResponse::RequestOnly(torrent),
                filename.to_string(),
            ))
            .await
        {
            error!("cannot send to torrent process: {}", err);
            return Ok(HttpResponse::InternalServerError().json(Failure {
                error: format!("cannot send to torrent process: {}", err),
            }));
        }
    }
    Ok(HttpResponse::Ok().into())
}

#[get("/oauth2/authorization/oidc")]
async fn authorize(oidc_client: web::Data<DiscoveredClient>) -> impl Responder {
    let auth_url = oidc_client.auth_url(&Options {
        scope: Some("email".into()),
        ..Default::default()
    });

    debug!("authorize: {}", auth_url);

    HttpResponse::Found()
        .header(http::header::LOCATION, auth_url.to_string())
        .finish()
}

#[get("/account")]
async fn account(user: User) -> impl Responder {
    web::Json(user)
}

#[derive(Deserialize, Debug)]
struct LoginQuery {
    code: String,
}

async fn request_token(
    oidc_client: web::Data<DiscoveredClient>,
    query: web::Query<LoginQuery>,
) -> Result<Option<(Token, Userinfo)>, ExitFailure> {
    let mut token: Token = oidc_client.request_token(&query.code).await?.into();
    if let Some(mut id_token) = token.id_token.as_mut() {
        oidc_client.decode_token(&mut id_token)?;
        oidc_client.validate_token(&id_token, None, None)?;
        debug!("token: {:?}", id_token);
    } else {
        return Ok(None);
    }
    let userinfo = oidc_client.request_userinfo(&token).await?;

    debug!("user info: {:?}", userinfo);
    Ok(Some((token, userinfo)))
}

#[get("/login/oauth2/code/oidc")]
async fn login(
    oidc_client: web::Data<DiscoveredClient>,
    query: web::Query<LoginQuery>,
    sessions: web::Data<RwLock<Sessions>>,
    identity: Identity,
) -> impl Responder {
    debug!("login: {:?}", query);

    match request_token(oidc_client, query).await {
        Ok(Some((token, userinfo))) => {
            let id = uuid::Uuid::new_v4().to_string();

            let login = userinfo.preferred_username.clone();
            let email = userinfo.email.clone();

            if email != Some(RSBT_ALLOW.to_string()) {
                error!("email {:?} is not allowed", email);
                return HttpResponse::Unauthorized().finish();
            }

            let user = User {
                id: userinfo.sub.clone(),
                login,
                last_name: userinfo.family_name.clone(),
                first_name: userinfo.name.clone(),
                email,
                activated: userinfo.email_verified,
                image_url: userinfo.picture.clone().map(|x| x.to_string()),
                lang_key: Some("en".to_string()),
                authorities: vec!["ROLE_USER".to_string()], //FIXME: read from token
            };

            identity.remember(id.clone());
            sessions
                .write()
                .await
                .map
                .insert(id, (user, token, userinfo));

            HttpResponse::Found()
                .header(http::header::LOCATION, host("/"))
                .finish()
        }
        Ok(None) => {
            error!("login error in call: no id_token found");

            HttpResponse::Unauthorized().finish()
        }
        Err(err) => {
            error!("login error in call: {:?}", err);

            HttpResponse::Unauthorized().finish()
        }
    }
}

#[post("/logout")]
async fn logout(
    oidc_client: web::Data<DiscoveredClient>,
    sessions: web::Data<RwLock<Sessions>>,
    identity: Identity,
) -> impl Responder {
    if let Some(id) = identity.identity() {
        identity.forget();
        if let Some((user, token, _userinfo)) = sessions.write().await.map.remove(&id) {
            debug!("logout user: {:?}", user);

            let id_token = token.bearer.access_token;
            let logout_url = oidc_client.config().end_session_endpoint.clone();

            return HttpResponse::Ok().json(Logout {
                id_token,
                logout_url,
            });
        }
    }

    HttpResponse::Unauthorized().finish()
}

fn host(path: &str) -> String {
    RSBT_UI_HOST.clone() + path
}

#[get("/stream")]
async fn stream(broadcaster: web::Data<RwLock<Broadcaster>>) -> impl Responder {
    let rx = broadcaster.write().await.new_client();
    HttpResponse::Ok()
        .content_type("text/event-stream")
        .keep_alive()
        .no_chunking()
        .streaming(rx)
}

#[actix_rt::main]
async fn main() -> Result<(), ExitFailure> {
    dotenv().ok();

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

    let settings = Settings::default();
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
            .service(login)
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

struct Broadcaster {
    clients: Vec<Sender<Bytes>>,
}

impl Broadcaster {
    fn new() -> Self {
        Self { clients: vec![] }
    }

    fn new_client(&mut self) -> Client {
        eprintln!("adding new client");
        let (tx, rx) = mpsc::channel(100);

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
