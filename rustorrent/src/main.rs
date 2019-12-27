use env_logger::Builder as LoggerBuilder;
use exitfailure::ExitFailure;
use failure::ResultExt;
use futures::future::lazy;
use futures::prelude::*;
use log::{debug, info, Level};
use rustorrent::app::RustorrentApp;
use rustorrent::types::Settings;

mod cli;

/// Port for client to listen for peer connections
///
/// If port is not available - took up to `PEER_PORT_MAX`
const PEER_PORT: u16 = 6881;
const PEER_PORT_MAX: u16 = 6889;

#[tokio::main]
async fn main() -> Result<(), ExitFailure> {
    let cli = cli::from_args();

    if let Some(level_filter) = cli.verbose.log_level().map(|x| x.to_level_filter()) {
        LoggerBuilder::new()
            .filter(
                Some(&env!("CARGO_PKG_NAME").replace("-", "_")),
                level_filter,
            )
            .filter(None, Level::Warn.to_level_filter())
            .try_init()?;
    }

    info!("starting torrent client");

    let settings = load_settings()?;

    let mut settings = settings.override_with(cli.config);

    debug!("calculated settings {:#?}", settings);

    let app = RustorrentApp::new(settings);

    app.download(cli.torrent).await?;

    Ok(())
}

fn load_settings() -> std::io::Result<Settings> {
    debug!("loading settings");

    confy::load(env!("CARGO_PKG_NAME"))
}
