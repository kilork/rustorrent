use super::*;
use crate::{errors::RustorrentError, PEER_ID};

use crate::{
    app::{DownloadTorrentEvent, TorrentProcess},
    types::{torrent::TrackerAnnounce, Settings},
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
        let try_interval_to_query_tracker = match proto {
            Announce::Http => {
                http::http_announce(settings.clone(), torrent_process.clone(), announce_url).await
            }
            Announce::Udp => {
                udp::udp_announce(settings.clone(), torrent_process.clone(), announce_url).await
            }
            _ => return Ok(()),
        };

        let interval_to_query_tracker = match try_interval_to_query_tracker {
            Ok(i) => i,
            Err(err) => {
                error!("announce loop error {:?}", err);
                Duration::from_secs(5)
            }
        };

        debug!("query tracker in {:?}", interval_to_query_tracker);

        delay_for(interval_to_query_tracker).await;
    }
}
