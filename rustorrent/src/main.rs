use exitfailure::ExitFailure;
use failure::ResultExt;
use log::{debug, info};
use rustorrent::app::RustorrentApp;
use rustorrent::types::Settings;

mod cli;

const PEER_PORT: u16 = 6881;
const PEER_PORT_MAX: u16 = 6889;

fn main() -> Result<(), ExitFailure> {
    let cli = cli::from_args();

    cli.verbose.setup_env_logger(env!("CARGO_PKG_NAME"))?;

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

    let mut app = RustorrentApp::new(settings);

    app.add_torrent_from_file(&cli.torrent)?;

    app.start()
        .with_context(|err| format!("Could not start app: {}", err))?;

    Ok(())
}

fn load_settings() -> Result<Settings, std::io::Error> {
    debug!("loading settings");

    confy::load(env!("CARGO_PKG_NAME"))
}
