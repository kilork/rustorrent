use exitfailure::ExitFailure;
use failure::ResultExt;
use log::{debug, info};
use rustorrent::{parse_torrent, types::Settings};

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
    }

    if config.port_max.is_none() {
        config.port_max = Some(PEER_PORT_MAX);
    }

    info!("downloading {:?}", cli.torrent.to_str());

    let mut buf = vec![];

    let torrent = parse_torrent(&cli.torrent, &mut buf)
        .with_context(|_| format!("could not parse torrent {:?}", &cli.torrent))?;

    let mut announce_buf = vec![];

    torrent.announce(&mut announce_buf).with_context(|_| {
        format!(
            "could not announce torrent to tracker {}",
            torrent.announce_url
        )
    })?;

    Ok(())
}

fn load_settings() -> Result<Settings, std::io::Error> {
    debug!("loading settings");

    confy::load(env!("CARGO_PKG_NAME"))
}
