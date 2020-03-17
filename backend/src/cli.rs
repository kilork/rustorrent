use rsbt_service::types::Config;
use structopt::StructOpt;

/// RSBT Web UI.
#[derive(StructOpt)]
pub(crate) struct Cli {
    /// Path to store configuration.
    #[structopt(long)]
    pub(crate) config_path: Option<String>,
    #[structopt(flatten)]
    pub(crate) config: Config,
    /// Do not save session cookie to storage.
    #[structopt(long)]
    pub(crate) no_session_cookie: bool,
}
