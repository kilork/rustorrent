use exitfailure::ExitFailure;
use failure::ResultExt;
use log::info;
use rustorrent::parse_torrent;

mod cli;

fn main() -> Result<(), ExitFailure> {
    let cli = cli::from_args();

    cli.verbose.setup_env_logger("rustorrent")?;

    info!("starting torrent client");

    parse_torrent(&cli.torrent)
        .with_context(|_| format!("could not parse torrent {:?}", &cli.torrent))?;
    println!("{:?}", cli.torrent.to_str());

    Ok(())
}
