use super::*;

#[derive(Debug, PartialEq)]
pub struct TorrentInfo {
    pub name: String,
    pub piece_length: i64,
    pub pieces: Vec<u8>,
    pub length: Option<i64>,
    pub files: Option<Vec<TorrentInfoFile>>,
}

#[derive(Debug, PartialEq)]
pub struct TorrentInfoFile {
    pub length: i64,
    pub path: Vec<String>,
}

impl TorrentInfo {
    pub fn len(&self) -> usize {
        if let Some(len) = self.length {
            len as usize
        } else if let Some(files) = &self.files {
            files.iter().map(|x| x.length).sum::<i64>() as usize
        } else {
            panic!("Wrong torrent info block");
        }
    }
}

try_from_bencode!(TorrentInfo,
    normal: (
        "name" => name,
        "piece length" => piece_length,
        "pieces" => pieces
    ),
    optional: (
        "length" => length,
        "files" => files
    ),
);

try_from_bencode!(TorrentInfoFile,
    normal: (
        "length" => length,
        "path" => path
    ),
);

impl TryFrom<BencodeBlob> for Vec<TorrentInfoFile> {
    type Error = TryFromBencode;

    fn try_from(blob: BencodeBlob) -> Result<Self, Self::Error> {
        match blob.value {
            BencodeValue::List(l) => Ok(l.into_iter().map(|x| x.try_into().unwrap()).collect()),
            _ => Err(TryFromBencode::NotList),
        }
    }
}
