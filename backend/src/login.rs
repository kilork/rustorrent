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
async fn login_get(
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