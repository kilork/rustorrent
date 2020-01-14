use super::*;
use crate::{errors::RustorrentError, types::torrent::parse_torrent, PEER_ID};

use crate::{
    app::{DownloadTorrentEvent, TorrentProcess},
    types::{
        info::TorrentInfo,
        message::{Message, MessageCodec},
        peer::{Handshake, Peer},
        torrent::{Torrent, TrackerAnnounce},
        Settings,
    },
    {commands::url_encode, count_parts, SHA1_SIZE},
};

mod http;
mod udp;

enum Announce {
    Http,
    Udp,
    WebSocket,
}

pub async fn announce_loop(
    settings: Arc<Settings>,
    torrent_process: Arc<TorrentProcess>,
) -> Result<(), RustorrentError> {
    let announce_url = &torrent_process.torrent.announce_url;
    let proto = if let Some(proto) = announce_url.split("://").next().map(|x| x.to_lowercase()) {
        match proto.as_str() {
            "http" | "https" => Announce::Http,
            "udp" => Announce::Udp,
            "wss" => Announce::WebSocket,
            _ => return Err(RustorrentError::AnnounceProtocolUnknown(proto)),
        }
    } else {
        return Err(RustorrentError::AnnounceProtocolFailure);
    };

    loop {
        let interval_to_query_tracker = match proto {
            Announce::Http => {
                http::http_announce(settings.clone(), torrent_process.clone(), announce_url).await?
            }
            Announce::Udp => {
                udp::udp_announce(settings.clone(), torrent_process.clone(), announce_url).await?
            }
            _ => return Ok(()),
        };

        debug!("query tracker in {:?}", interval_to_query_tracker);

        delay_for(interval_to_query_tracker).await;
    }
}
