use super::*;
use rsbt_service::types::Properties;

pub(crate) struct Sessions {
    storage_path: Option<PathBuf>,
    pub(crate) map: HashMap<String, SessionUser>,
}

impl Sessions {
    pub(crate) async fn new(properties: &Properties) -> Result<Self, ExitFailure> {
        Ok(Self {
            storage_path: None,
            map: HashMap::new(),
        })
    }
}
#[derive(Serialize, Deserialize)]
pub(crate) struct SessionUser {
    pub(crate) user: User,
    pub(crate) access_token: String,
    pub(crate) info: Userinfo,
}
