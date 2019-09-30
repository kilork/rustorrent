use super::*;

use hyper::{Client, Uri};

impl Inner {
    pub(crate) fn command_start_announce_process(
        self: Arc<Self>,
        torrent_process: Arc<TorrentProcess>,
    ) -> Result<(), RustorrentError> {
        {
            let mut announce_state = torrent_process.announce_state.lock().unwrap();
            match *announce_state {
                AnnounceState::Idle => {
                    *announce_state = AnnounceState::Request;
                }
                _ => {
                    debug!("torrent process announce already running");
                    return Ok(());
                }
            }
        }

        let client = Client::new();

        let mut url = {
            let stats = torrent_process.stats.lock().unwrap();
            format!(
                "{}?info_hash={}&peer_id={}&left={}",
                torrent_process.torrent.announce_url,
                url_encode(&torrent_process.hash_id[..]),
                url_encode(&PEER_ID[..]),
                stats.left
            )
        };

        let config = &self.settings.config;

        if let Some(port) = config.port {
            url += format!("&port={}", port).as_str();
        }

        if let Some(compact) = config.compact {
            url += format!("&compact={}", if compact { 1 } else { 0 }).as_str();
        }

        debug!("Get tracker announce from: {}", url);

        let announce_state_succ = torrent_process.announce_state.clone();
        let announce_state_err = torrent_process.announce_state.clone();

        let this_response = self.clone();
        let this_err = self.clone();
        let torrent_process_response = torrent_process.clone();
        let torrent_process_err = torrent_process.clone();

        let uri = url.parse().unwrap();
        let process = client
            .get(uri)
            .and_then(|res| {
                debug!("Result code: {}", res.status());

                res.into_body().concat2()
            })
            .and_then(|body| {
                let mut buf = vec![];
                let mut body = std::io::Cursor::new(body);
                std::io::copy(&mut body, &mut buf).unwrap();
                Ok(buf)
            })
            .map_err(RustorrentError::from)
            .and_then(move |response| {
                debug!(
                    "Tracker response (url encoded): {}",
                    percent_encode(&response, NON_ALPHANUMERIC).to_string()
                );
                let tracker_announce: TrackerAnnounce = response.try_into()?;
                debug!("Tracker response parsed: {:#?}", tracker_announce);
                *announce_state_succ.lock().unwrap() = AnnounceState::Idle;
                let process_announce =
                    RustorrentCommand::ProcessAnnounce(torrent_process_response, tracker_announce);
                this_response.send_command(process_announce)?;
                Ok(())
            })
            .map_err(move |err| {
                error!("Error in announce request: {}", err);
                let err = Arc::new(err);
                *announce_state_err.lock().unwrap() = AnnounceState::Error(err.clone());
                let process_announce =
                    RustorrentCommand::ProcessAnnounceError(torrent_process_err, err);
                this_err
                    .send_command(process_announce)
                    .map_err(|err| error!("Cannot send process announce error: {}", err))
                    .unwrap();
            });
        tokio::spawn(process);
        Ok(())
    }
}
