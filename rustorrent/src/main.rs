use exitfailure::ExitFailure;
use failure::ResultExt;
use log::info;
use rustorrent::parse_torrent;

mod cli;

const PEER_PORT: u16 = 6881;
const PEER_PORT_MAX: u16 = 6889;

fn main() -> Result<(), ExitFailure> {
    let cli = cli::from_args();

    cli.verbose.setup_env_logger("rustorrent")?;

    info!("starting torrent client");

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
