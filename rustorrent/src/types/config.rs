use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use std::net::Ipv4Addr;

/// Data to be both passed as arguments and in form of config file
#[derive(StructOpt, Serialize, Deserialize, Debug)]
pub struct Config {
    /// Forces compact parameter behavior for announce request
    ///
    /// Default behavior is to not set compact parameter relying on default server configuration.
    /// To force compact=1 use true value. To force compact=0 use false value.
    #[structopt(long)]
    pub compact: Option<bool>,
    /// IPv4 address to listen to
    #[structopt(long)]
    pub ipv4: Option<Ipv4Addr>,
    /// IPv4 port to listen on
    #[structopt(long)]
    pub port: Option<u16>,
    /// IPv4 max port
    ///
    /// If there is no free port between port and port-max - client will exit with exception.
    #[structopt(long = "port-max")]
    pub port_max: Option<u16>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            compact: None,
            ipv4: None,
            port: None,
            port_max: None,
        }
    }
}

/// Global application settings
#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Settings {
    pub config: Config,
}

impl Settings {
    pub fn override_with(&self, config: &Config) -> Self {
        Self {
            config: Config {
                compact: config.compact.or(self.config.compact),
                ipv4: config.ipv4.or(self.config.ipv4),
                port: config.port.or(self.config.port),
                port_max: config.port_max.or(self.config.port_max),
            },
        }
    }
}
