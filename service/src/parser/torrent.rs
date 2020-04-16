use crate::{types::Torrent, RsbtError};
use std::convert::TryInto;

pub fn parse_torrent(buf: &[u8]) -> Result<Torrent, RsbtError> {
    let torrent = buf.try_into()?;

    Ok(torrent)
}

#[cfg(test)]
mod tests {
    use super::Torrent;
    use std::convert::TryInto;

    #[test]
    fn parse_torrent() {
        let torrent_bytes = b"d8:announce36:http://bt1.archive.org:6969/announce13:announce-listll36:http://bt1.archive.org:6969/announceel36:http://bt2.archive.org:6969/announceee4:infoi1ee";
        let _torrent: Torrent = torrent_bytes.to_vec().try_into().unwrap();
    }
}
