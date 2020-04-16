use clap_verbosity_flag::Verbosity;
use rsbt_service::RsbtConfig;
use std::path::PathBuf;
use structopt::StructOpt;

/// Extremely fast and simple torrent client
#[derive(StructOpt)]
pub(crate) struct Cli {
    /// Path to torrent
    #[structopt(parse(from_os_str))]
    pub torrent: PathBuf,
    #[structopt(flatten)]
    pub verbose: Verbosity,
    #[structopt(flatten)]
    pub config: RsbtConfig,
}

pub(crate) fn from_args() -> Cli {
    Cli::from_args()
}
