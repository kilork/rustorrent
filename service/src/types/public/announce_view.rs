use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct AnnounceView {
    pub(crate) url: String,
}
