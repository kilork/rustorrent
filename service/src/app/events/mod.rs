use super::*;

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TorrentEvent {
    Storage {},
}
