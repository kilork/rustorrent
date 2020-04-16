use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct FileView {
    pub id: usize,
    pub name: String,
    pub saved: usize,
    pub size: usize,
}
