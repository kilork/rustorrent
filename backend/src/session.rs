use crate::login::User;
use exitfailure::ExitFailure;
use openid::Userinfo;
use rsbt_service::RsbtProperties;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
use tokio::sync::RwLock;

pub(crate) struct Sessions {
    local: bool,
    storage_path: Option<PathBuf>,
    pub(crate) map: RwLock<HashMap<String, SessionUser>>,
}

impl Sessions {
    pub(crate) async fn new(properties: &RsbtProperties, local: bool) -> Result<Self, ExitFailure> {
        Ok(Self {
            local,
            storage_path: None,
            map: RwLock::new(HashMap::new()),
        })
    }

    pub(crate) fn is_local(&self) -> bool {
        self.local
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SessionUser {
    pub(crate) user: User,
    pub(crate) access_token: String,
    pub(crate) info: Userinfo,
}
