use super::*;

#[derive(Debug)]
pub struct TorrentStorage {
    pub pieces: Vec<TorrentPiece>,
    pub pieces_left: usize,
    pub downloaded: Vec<u8>,
    pub bytes_downloaded: usize,
    pub bytes_uploaded: usize,
}

impl TorrentStorage {
    pub async fn save(&mut self, index: usize, data: Vec<u8>) -> Result<(), RustorrentError> {
        while self.pieces.len() <= index {
            self.pieces.push(TorrentPiece(None));
        }

        if let TorrentPiece(None) = self.pieces[index] {
            self.pieces_left -= 1;
            self.bytes_downloaded += data.len();
        }

        self.pieces[index] = TorrentPiece(Some(data));

        let (index, bit) = crate::messages::index_in_bitarray(index);
        while self.downloaded.len() <= index {
            self.downloaded.push(0);
        }
        self.downloaded[index] |= bit;

        Ok(())
    }
}

#[derive(Debug)]
pub struct TorrentPiece(Option<Vec<u8>>);
