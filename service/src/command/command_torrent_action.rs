use crate::types::public::TorrentAction;

#[derive(Debug)]
pub struct CommandTorrentAction {
    pub id: usize,
    pub action: TorrentAction,
}
