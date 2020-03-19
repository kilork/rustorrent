use super::*;
use rsbt_service::types::Properties;

pub(crate) struct Sessions {
    local: bool,
    storage_path: Option<PathBuf>,
    pub(crate) map: RwLock<HashMap<String, SessionUser>>,
}

impl Sessions {
    pub(crate) async fn new(properties: &Properties, local: bool) -> Result<Self, ExitFailure> {
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
