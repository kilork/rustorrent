use crate::{
    announce::{http, udp, Announce},
    errors::RsbtError,
    process::TorrentToken,
    types::Properties,
};
use log::{debug, error};
use std::{sync::Arc, time::Duration};
use tokio::time::delay_for;

pub async fn announce_loop(
    properties: Arc<Properties>,
    torrent_process: Arc<TorrentToken>,
) -> Result<(), RsbtError> {
    debug!("announce-list present, proceed to new implementation");

    Ok(())
}
