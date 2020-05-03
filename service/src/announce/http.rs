use crate::{
    announce::Announcement,
    errors::RsbtError,
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
) -> Result<Announcement, RsbtError> {
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
            return Err(RsbtError::TorrentHttpAnnounceBadResponse(format!(
                "{:?}",
                bad_result
            )));
        }
        Err(err) => {
            error!("Failure {}, retry in 5 seconds", err);
            return Err(RsbtError::TorrentHttpAnnounceFailure(err));
        }
    };

    let mut announce_data = result.into_body();

    let mut announce_bytes = vec![];

    while let Some(chunk) = announce_data.data().await {
        announce_bytes.append(&mut chunk?.to_vec());
    }

    let tracker_announce: TrackerAnnounce = announce_bytes.try_into()?;
    let requery_interval = Duration::from_secs(tracker_announce.interval as u64);

    debug!("Tracker announce: {:?}", tracker_announce);

    Ok(Announcement {
        requery_interval,
        peers: tracker_announce.peers,
    })
}
