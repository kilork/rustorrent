use super::*;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct User {
    pub(crate) id: String,
    pub(crate) login: Option<String>,
    pub(crate) first_name: Option<String>,
    pub(crate) last_name: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) image_url: Option<String>,
    pub(crate) activated: bool,
    pub(crate) lang_key: Option<String>,
    pub(crate) authorities: Vec<String>,
}

lazy_static::lazy_static! {
    static ref LOCAL_USER: User = User {
        id: "0".into(),
        login: Some("root".into()),
        email: Some("root@localhost".into()),
        lang_key: Some("en".into()),
        activated: true,
        authorities: vec!["ROLE_USER".into(), "ROLE_LOCAL".into()],
        ..Default::default()
    };
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Logout {
    pub(crate) id_token: String,
    pub(crate) logout_url: Option<Url>,
}

impl FromRequest for User {
    type Config = ();
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<User, Error>>>>;

    fn from_request(req: &HttpRequest, pl: &mut Payload) -> Self::Future {
        let sessions: Option<&web::Data<Sessions>> = req.app_data();

        if sessions.is_none() {
            error!("sessions is none!");
            return Box::pin(async { Err(ErrorUnauthorized("unauthorized")) });
        }

        let sessions = sessions.cloned().unwrap();

        if sessions.is_local() {
            return Box::pin(async { Ok(LOCAL_USER.clone()) });
        }

        let fut = Identity::from_request(req, pl);

        Box::pin(async move {
            if let Some(identity) = fut.await?.identity() {
                if let Some(user) = sessions
                    .map
                    .read()
                    .await
                    .get(&identity)
                    .map(|x| x.user.clone())
                {
                    return Ok(user);
                }
            };

            Err(ErrorUnauthorized("unauthorized"))
        })
    }
}

#[derive(Deserialize)]
struct AuthorizeQuery {
    state: Option<String>,
}

#[get("/oauth2/authorization/oidc")]
async fn authorize(
    oidc_client: web::Data<DiscoveredClient>,
    authorize_query: web::Query<AuthorizeQuery>,
) -> impl Responder {
    let auth_url = oidc_client.auth_url(&Options {
        scope: Some("email".into()),
        state: authorize_query.state.clone(),
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
    state: Option<String>,
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
async fn login_get(
    oidc_client: web::Data<DiscoveredClient>,
    login_query: web::Query<LoginQuery>,
    sessions: web::Data<Sessions>,
    identity: Identity,
) -> impl Responder {
    debug!("login: {:?}", login_query);

    let state = login_query.state.clone();
    match request_token(oidc_client, login_query).await {
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
            sessions.map.write().await.insert(
                id,
                SessionUser {
                    user,
                    access_token: token.bearer.access_token.clone(),
                    info: userinfo,
                },
            );

            HttpResponse::Found()
                .header(
                    http::header::LOCATION,
                    host(&state.unwrap_or_else(|| "/".into())),
                )
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
    sessions: web::Data<Sessions>,
    identity: Identity,
) -> impl Responder {
    if let Some(id) = identity.identity() {
        identity.forget();
        if let Some(SessionUser {
            user, access_token, ..
        }) = sessions.map.write().await.remove(&id)
        {
            debug!("logout user: {:?}", user);

            let id_token = access_token;
            let logout_url = oidc_client.config().end_session_endpoint.clone();

            return HttpResponse::Ok().json(Logout {
                id_token,
                logout_url,
            });
        }
    }

    HttpResponse::Unauthorized().finish()
}

pub(crate) async fn connect_to_openid_provider() -> Result<DiscoveredClient, ExitFailure> {
    let client_id = RSBT_OPENID_CLIENT_ID.to_string();
    let client_secret = RSBT_OPENID_CLIENT_SECRET.to_string();
    let redirect = Some(host("/login/oauth2/code/oidc"));
    let issuer = reqwest::Url::parse(RSBT_OPENID_ISSUER.as_str())?;
    debug!("redirect: {:?}", redirect);
    debug!("issuer: {}", issuer);
    let client = openid::Client::discover(client_id, client_secret, redirect, issuer).await?;

    debug!("discovered config: {:?}", client.config());

    Ok(client)
}
