use env_logger::Builder as LoggerBuilder;
use exitfailure::ExitFailure;
use log::{debug, info, Level};
use rsbt_service::{RsbtApp, RsbtProperties, RsbtSettings};

mod cli;

/// Port for client to listen for peer connections
///
/// If port is not available - took up to `PEER_PORT_MAX`
// FIXME:const PEER_PORT: u16 = 6881;
// FIXME:const PEER_PORT_MAX: u16 = 6889;

#[tokio::main]
async fn main() -> Result<(), ExitFailure> {
    let cli = cli::from_args();

    if let Some(level_filter) = cli.verbose.log_level().map(|x| x.to_level_filter()) {
        LoggerBuilder::new()
            .filter(
                Some(&env!("CARGO_PKG_NAME").replace("-", "_")),
                level_filter,
            )
            .filter(Some("flat_storage_mmap"), level_filter)
            .filter(None, Level::Warn.to_level_filter())
            .try_init()?;
    }

    info!("starting torrent client");

    let properties: RsbtProperties = (
        load_settings()?.override_with(cli.config),
        rsbt_service::default_app_dir(),
    )
        .into();

    debug!("calculated properties {:#?}", properties);

    let mut app = RsbtApp::new(properties);

    app.download(cli.torrent).await?;

    Ok(())
}

fn load_settings() -> Result<RsbtSettings, confy::ConfyError> {
    debug!("loading settings");

    confy::load(env!("CARGO_PKG_NAME"))
}
