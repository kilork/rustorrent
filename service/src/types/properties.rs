use super::*;

use std::{
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
};

#[derive(Debug)]
pub struct Properties {
    pub compact: Option<bool>,
    /// Address to listen to
    pub listen: IpAddr,
    /// Port to listen on
    pub port: u16,
    /// Max port
    ///
    /// If there is no free port between port and port-max - client will exit with exception.
    pub port_max: u16,
    /// Download path
    pub save_to: PathBuf,
    /// Storage path
    pub storage: PathBuf,
    /// Config path
    pub config_dir: PathBuf,
}

impl From<(Settings, PathBuf)> for Properties {
    fn from(value: (Settings, PathBuf)) -> Self {
        let config = value.0.config;
        let config_dir = value.1;
        let (save_to, storage) = match (
            config.save_to.map(PathBuf::from),
            config.storage.map(PathBuf::from),
        ) {
            (Some(save_to), Some(storage)) => (save_to, storage),
            (Some(save_to), None) => (save_to.clone(), save_to),
            (None, Some(storage)) => (config_dir.join("download"), storage),
            (None, None) => (config_dir.join("download"), config_dir.join("download")),
        };
        Self {
            compact: config.compact,
            listen: config
                .listen
                .unwrap_or_else(|| IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))),
            port: config.port,
            port_max: config.port_max,
            save_to,
            storage,
            config_dir,
        }
    }
}
