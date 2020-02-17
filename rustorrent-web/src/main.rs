#[macro_use]
extern crate actix_web;

use actix::prelude::*;
use actix_identity::{CookieIdentityPolicy, Identity, IdentityService};
use actix_service::Service;
use actix_web::{
    dev::Payload, error::ErrorUnauthorized, http, middleware, web, App, Error, FromRequest,
    HttpRequest, HttpResponse, HttpServer, Responder,
};
use actix_web_static_files;
use bytes::Bytes;
use exitfailure::ExitFailure;
use inth_oauth2::token::Token;
use log::{debug, error, info};
use oidc;
use reqwest;
use rustorrent_web_resources::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    pin::Pin,
    sync::RwLock,
    task::{Context, Poll},
};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task,
    time::{interval_at, Duration, Instant},
};
use url::Url;

lazy_static::lazy_static! {
pub  static ref RUSTORRENT_HOST: String = std::env::var("RUSTORRENT_HOST")
                                            .unwrap_or_else(|_| "http://localhost:8080".to_string());
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
                    .unwrap()
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
    map: HashMap<String, (User, oidc::token::Token, oidc::Userinfo)>,
}

#[get("/torrents")]
async fn torrent_list() -> impl Responder {
    web::Json("")
}

#[get("/oauth2/authorization/oidc")]
async fn authorize(oidc_client: web::Data<oidc::Client>) -> impl Responder {
    // 302 Location: http://keycloak:9080/auth/realms/jhipster/protocol/openid-connect/auth?response_type=code&client_id=web_app&scope=openid%20address%20email%20jhipster%20microprofile-jwt%20offline_access%20phone%20profile%20roles%20web-origins&state=EP8ZhX1y0SsEyARdX3HUROfkbk2G1xvtfhChN2ujdsU%3D&redirect_uri=http://localhost:8080/login/oauth2/code/oidc&nonce=zPWbNSrAfBM5rg_uHjQ5Sb5ESusQUlMndIglOvxKVt0
    // http://keycloak:9080/auth/realms/jhipster/protocol/openid-connect/auth?response_type=code&client_id=web_app&scope=openid%20address%20email%20jhipster%20microprofile-jwt%20offline_access%20phone%20profile%20roles%20web-origins&redirect_uri=http://localhost:8080/login/oauth2/code/oidc
    // http://localhost:8080/login/oauth2/code/oidc?state=EP8ZhX1y0SsEyARdX3HUROfkbk2G1xvtfhChN2ujdsU%3D&session_state=1c1e04a6-2a99-4018-b77e-2fc653a5e333&code=028070ae-cf3b-4934-a7a9-8c4b25e16247.1c1e04a6-2a99-4018-b77e-2fc653a5e333.1eabef67-6473-4ba8-b07c-14bdbae4aaed
    // 302 http://localhost:8080
    let auth_url = oidc_client.auth_url(&Default::default());
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

#[get("/login/oauth2/code/oidc")]
async fn login(
    oidc_client: web::Data<oidc::Client>,
    query: web::Query<LoginQuery>,
    sessions: web::Data<RwLock<Sessions>>,
    identity: Identity,
) -> impl Responder {
    debug!("login: {:?}", query);
    match task::spawn_blocking(move || {
        let http = reqwest::blocking::Client::new();
        let mut token = oidc_client.request_token(&http, &query.code)?;
        oidc_client.decode_token(&mut token.id_token)?;
        oidc_client.validate_token(&token.id_token, None, None)?;
        let userinfo = oidc_client.request_userinfo(&http, &token)?;
        debug!("user info: {:?}", userinfo);
        debug!("token: {:?}", token.id_token);
        Ok::<(oidc::token::Token, oidc::Userinfo), ExitFailure>((token, userinfo))
    })
    .await
    {
        Ok(Ok((token, userinfo))) => {
            let id = uuid::Uuid::new_v4().to_string();

            let user = User {
                id: userinfo.sub.clone(),
                login: userinfo.preferred_username.clone(),
                last_name: userinfo.family_name.clone(),
                first_name: userinfo.name.clone(),
                email: userinfo.email.clone(),
                activated: userinfo.email_verified,
                image_url: userinfo.picture.clone().map(|x| x.to_string()),
                lang_key: Some("en".to_string()),
                authorities: vec!["ROLE_USER".to_string()], //FIXME: read from token
            };

            identity.remember(id.clone());
            sessions
                .write()
                .unwrap()
                .map
                .insert(id, (user, token, userinfo));

            HttpResponse::Found()
                .header(http::header::LOCATION, host("/"))
                .finish()
        }
        Ok(Err(err)) => {
            error!("login error 1: {:?}", err);
            HttpResponse::Unauthorized().finish()
        }
        Err(err) => {
            error!("login error 2: {:?}", err);
            HttpResponse::Unauthorized().finish()
        }
    }
}

#[post("/logout")]
async fn logout(
    oidc_client: web::Data<oidc::Client>,
    sessions: web::Data<RwLock<Sessions>>,
    identity: Identity,
) -> impl Responder {
    if let Some(id) = identity.identity() {
        identity.forget();
        if let Some((user, token, _userinfo)) = sessions.write().unwrap().map.remove(&id) {
            debug!("logout user: {:?}", user);
            let id_token = token.access_token().into();
            let logout_url = oidc_client.config().end_session_endpoint.clone();
            return HttpResponse::Ok().json(Logout { id_token, logout_url });
        }
    }
    HttpResponse::Unauthorized().finish()
}

fn host(path: &str) -> String {
    RUSTORRENT_HOST.clone() + path
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
    let client = web::Data::new(
        task::spawn_blocking(move || {
            let client_id = "web_app".to_string();
            let client_secret = "web_app".to_string();
            let redirect = reqwest::Url::parse(&host("/login/oauth2/code/oidc"))?;
            let issuer = reqwest::Url::parse("http://keycloak:9080/auth/realms/jhipster")?;
            let client = oidc::Client::discover(client_id, client_secret, redirect, issuer)?;
            debug!("discovered config: {:?}", client.config());
            Ok::<oidc::Client, ExitFailure>(client)
        })
        .await??,
    );

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
            .wrap(middleware::Logger::default())
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(&[0; 32])
                    .name("auth-rustorrent")
                    .secure(false),
            ))
            .app_data(broadcaster.clone())
            .app_data(client.clone())
            .app_data(sessions.clone())
            .service(torrent_list)
            .service(authorize)
            .service(login)
            .service(
                web::scope("/api")
                    .wrap_fn(|req, srv| {
                        let fut = srv.call(req);
                        async {
                            let mut res = fut.await?;
                            Ok(res)
                        }
                    })
                    .service(account)
                    .service(logout)
                    .service(stream),
            )
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
