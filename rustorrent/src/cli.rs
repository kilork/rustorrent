use clap_verbosity_flag::Verbosity;
use std::path::PathBuf;
use structopt::StructOpt;
use rustorrent::types::Config;

/// Extremely fast and simple torrent client
#[derive(StructOpt)]
pub(crate) struct Cli {
    /// Path to torrent
    #[structopt(parse(from_os_str))]
    pub torrent: PathBuf,
    #[structopt(flatten)]
    pub verbose: Verbosity,
    #[structopt(flatten)]
    pub config: Config,
}

pub(crate) fn from_args() -> Cli {
    Cli::from_args()
}
