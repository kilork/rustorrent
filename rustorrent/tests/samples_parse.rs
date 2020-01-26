use flat_storage::FlatStorageFile as TorrentInfoFile;
use rustorrent::types::torrent::Torrent;
use rustorrent::RustorrentError;

use std::convert::TryInto;
use std::path::PathBuf;

#[test]
fn parse_plan_9_torrent() -> Result<(), RustorrentError> {
    let torrent_bytes = include_bytes!("Plan_9_from_Outer_Space_1959_archive.torrent");
    let torrent: Torrent = torrent_bytes.to_vec().try_into()?;

    let info = torrent.info()?;

    let files = &info.files;
    assert_eq!(
        files,
        &vec![
            TorrentInfoFile {
                length: 383971,
                path: "Plan_9_from_Outer_Space_1959.asr.js".into(),
            },
            TorrentInfoFile {
                length: 51637,
                path: "Plan_9_from_Outer_Space_1959.asr.srt".into(),
            },
            TorrentInfoFile {
                length: 346429,
                path: "Plan_9_from_Outer_Space_1959.gif".into(),
            },
            TorrentInfoFile {
                length: 56478797,
                path: "Plan_9_from_Outer_Space_1959.mp3".into(),
            },
            TorrentInfoFile {
                length: 758756235,
                path: "Plan_9_from_Outer_Space_1959.mp4".into(),
            },
            TorrentInfoFile {
                length: 390383680,
                path: "Plan_9_from_Outer_Space_1959.ogv".into(),
            },
            TorrentInfoFile {
                length: 11287,
                path: "Plan_9_from_Outer_Space_1959.png".into(),
            },
            TorrentInfoFile {
                length: 293299508,
                path: "Plan_9_from_Outer_Space_1959_512kb.mp4".into(),
            },
            TorrentInfoFile {
                length: 4675,
                path: "Plan_9_from_Outer_Space_1959_meta.xml".into(),
            },
            TorrentInfoFile {
                length: 3209,
                path: "__ia_thumb.jpg".into(),
            },
        ]
    );

    let total_len = 383971
        + 51637
        + 346429
        + 56478797
        + 758756235
        + 390383680
        + 11287
        + 293299508
        + 4675
        + 3209;

    assert_eq!(info.len(), total_len);

    assert_eq!(info.pieces.len(), 1431);

    Ok(())
}

#[test]
fn parse_ferris_torrent() -> Result<(), RustorrentError> {
    let torrent_bytes = include_bytes!("ferris.gif.torrent");
    let torrent: Torrent = torrent_bytes.to_vec().try_into()?;

    let info = torrent.info()?;

    assert_eq!(349133, info.len());
    assert_eq!(
        Some(&PathBuf::from("ferris.gif")),
        info.files.iter().next().map(|x| &x.path)
    );

    Ok(())
}
