use env_logger::Builder as LoggerBuilder;
use exitfailure::ExitFailure;
use failure::ResultExt;
use futures::Future;
use log::{debug, info, Level};
use rustorrent::app::RustorrentApp;
use rustorrent::types::Settings;
use tokio::prelude::future::lazy;

mod cli;

/// Port for client to listen for peer connections
///
/// If port is not available - took up to `PEER_PORT_MAX`
const PEER_PORT: u16 = 6881;
const PEER_PORT_MAX: u16 = 6889;

fn main() -> Result<(), ExitFailure> {
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

    let mut settings = settings.override_with(&cli.config);
    let mut config = &mut settings.config;

    if config.port.is_none() {
        config.port = Some(PEER_PORT);
        if config.port_max.is_none() {
            config.port_max = Some(PEER_PORT_MAX);
        }
    } else if config.port_max.is_none() {
        config.port_max = config.port;
    }

    assert!(
        config.port <= config.port_max,
        "Max port must be greater than starting port"
    );

    debug!("calculated settings {:#?}", settings);

    let mut rt = tokio::runtime::Runtime::new()?;

    rt.block_on(lazy(move || -> Result<(), ExitFailure> {
        let mut app = RustorrentApp::new(settings);

        app.clone()
            .add_torrent_from_file(&cli.torrent)
            .with_context(|_| format!("Could not add torrent {:?}", cli.torrent))?;

        Ok(app.run().wait()?)
    }))?;

    Ok(())
}

fn load_settings() -> std::io::Result<Settings> {
    debug!("loading settings");

    confy::load(env!("CARGO_PKG_NAME"))
}
