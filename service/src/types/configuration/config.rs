use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use std::net::IpAddr;

const PEER_PORT: &str = "6881";
const PEER_PORT_MAX: &str = "6889";

/// Data to be both passed as arguments and in form of config file
#[derive(Default, StructOpt, Serialize, Deserialize, Debug)]
pub struct Config {
    /// Forces compact parameter behavior for announce request
    ///
    /// Default behavior is to not set compact parameter relying on default server configuration.
    /// To force compact=1 use true value. To force compact=0 use false value.
    #[structopt(long)]
    pub compact: Option<bool>,
    /// Address to listen to
    #[structopt(long)]
    pub listen: Option<IpAddr>,
    /// Port to listen on
    #[structopt(long, env = "RSBT_PEER_PORT", default_value = PEER_PORT)]
    pub port: u16,
    /// Max port
    ///
    /// If there is no free port between port and port-max - client will exit with exception.
    #[structopt(long, env = "RSBT_PEER_PORT_MAX", default_value = PEER_PORT_MAX)]
    pub port_max: u16,

    /// Download path
    #[structopt(long, env = "RSBT_PATH_DOWNLOAD")]
    pub save_to: Option<String>,
    /// Storage path
    #[structopt(long, env = "RSBT_PATH_STORAGE")]
    pub storage: Option<String>,
}

/// Global application settings
#[derive(Default, Serialize, Deserialize, Debug)]
pub struct Settings {
    pub config: Config,
    pub peers: Peers,
}

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct Peers {
    /// Maximum count of connections with peers
    max_connections: Option<usize>,
}

impl Settings {
    pub fn override_with(self, config: Config) -> Self {
        Self { config, ..self }
    }
}

// impl Default for
