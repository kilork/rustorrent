use crate::{
    errors::RsbtError,
    event::TorrentEvent,
    process::TorrentToken,
    types::{Properties, TrackerAnnounce},
    PEER_ID,
};
use http_body::Body;
use hyper::Client;
use log::{debug, error};
use percent_encoding::percent_encode_byte;
use std::{convert::TryInto, sync::Arc, time::Duration};

fn url_encode(data: &[u8]) -> String {
    data.iter()
        .map(|&x| percent_encode_byte(x))
        .collect::<String>()
}

pub(crate) async fn http_announce(
    properties: Arc<Properties>,
    torrent_process: Arc<TorrentToken>,
    announce_url: &str,
) -> Result<Duration, RsbtError> {
    let client: Client<_> = Client::new();

    let left = torrent_process.info.len();
    let mut url = {
        format!(
            "{}?info_hash={}&peer_id={}&left={}&port={}",
            announce_url,
            url_encode(&torrent_process.hash_id[..]),
            url_encode(&PEER_ID[..]),
            left,
            properties.port,
        )
    };

    if let Some(compact) = properties.compact {
        url += &format!("&compact={}", if compact { 1 } else { 0 });
    }

    let uri = url.parse()?;
    let res = client.get(uri).await;

    debug!("Got tracker announce from: {}", url);

    let result = match res {
        Ok(result) if result.status().is_success() => result,
        Ok(bad_result) => {
            error!(
                "Bad response from tracker: {:?}, retry in 5 seconds...",
                bad_result
            );
            return Ok(Duration::from_secs(5));
        }
        Err(err) => {
            error!("Failure {}, retry in 5 seconds", err);
            return Ok(Duration::from_secs(5));
        }
    };

    let mut announce_data = result.into_body();

    let mut announce_bytes = vec![];

    while let Some(chunk) = announce_data.data().await {
        announce_bytes.append(&mut chunk?.to_vec());
    }

    let tracker_announce: Result<TrackerAnnounce, _> = announce_bytes.try_into();

    let interval_to_query_tracker = match tracker_announce {
        Ok(tracker_announce) => {
            let interval_to_reannounce = tracker_announce.interval.try_into()?;

            debug!("Tracker announce: {:?}", tracker_announce);

            torrent_process
                .broker_sender
                .clone()
                .send(TorrentEvent::Announce(tracker_announce.peers))
                .await?;
            Duration::from_secs(interval_to_reannounce)
        }

        Err(err) => {
            error!("Failure {}, retry in 5 seconds", err);
            Duration::from_secs(5)
        }
    };

    Ok(interval_to_query_tracker)
}
