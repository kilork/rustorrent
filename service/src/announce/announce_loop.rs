use crate::{
    announce::{http, udp, Announce},
    errors::RsbtError,
    event::TorrentEvent,
    process::TorrentToken,
    types::Properties,
    PEER_ID,
};
use log::{debug, error};
use std::{sync::Arc, time::Duration};
use tokio::time::delay_for;

pub async fn announce_loop(
    properties: Arc<Properties>,
    torrent_process: Arc<TorrentToken>,
) -> Result<(), RsbtError> {
    let announce_url = &torrent_process.torrent.announce_url;
    let proto = if let Some(proto) = announce_url.split("://").next().map(|x| x.to_lowercase()) {
        match proto.as_str() {
            "http" | "https" => Announce::Http,
            "udp" => Announce::Udp,
            "wss" => Announce::WebSocket,
            _ => return Err(RsbtError::AnnounceProtocolUnknown(proto)),
        }
    } else {
        return Err(RsbtError::AnnounceProtocolFailure);
    };

    loop {
        let try_interval_to_query_tracker = match proto {
            Announce::Http => {
                http::http_announce(properties.clone(), torrent_process.clone(), announce_url).await
            }
            Announce::Udp => {
                udp::udp_announce(properties.clone(), torrent_process.clone(), announce_url).await
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
