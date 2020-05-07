use crate::types::udp_tracker::{
    UdpTrackerAuthentication, UdpTrackerRequestData, UdpTrackerResponse, UdpTrackerResponseData,
};
use crate::{process::TorrentTokenProvider, types::configuration::PropertiesProvider};
use rand::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct UdpTrackerRequest {
    /// Must be initialized to 0x41727101980 in network byte order for connect.
    /// This will identify the protocol.
    pub(crate) connection_id: i64,
    pub(crate) transaction_id: i32,
    pub(crate) data: UdpTrackerRequestData,
    pub(crate) authentication: Option<UdpTrackerAuthentication>,
    pub(crate) request_string: Option<String>,
}

impl UdpTrackerRequest {
    pub(crate) fn connect() -> Self {
        UdpTrackerRequest {
            connection_id: 0x0417_2710_1980,
            transaction_id: random(),
            data: UdpTrackerRequestData::Connect,
            authentication: None,
            request_string: None,
        }
    }

    pub(crate) fn announce<P: PropertiesProvider, TT: TorrentTokenProvider>(
        connection_id: i64,
        properties: Arc<P>,
        torrent_process: Arc<TT>,
    ) -> Self {
        let left = torrent_process.info().len() as i64;

        Self {
            connection_id,
            transaction_id: random(),
            data: UdpTrackerRequestData::Announce {
                info_hash: torrent_process.hash_id().clone(),
                peer_id: crate::PEER_ID,
                downloaded: 0,
                uploaded: 0,
                left,
                event: 0,
                ip: 0,
                extensions: 0,
                num_want: -1,
                key: random(),
                port: properties.port(),
            },
            authentication: None,
            request_string: None,
        }
    }

    pub(crate) fn match_response(&self, response: &UdpTrackerResponse) -> bool {
        match (self, response) {
            (
                UdpTrackerRequest {
                    transaction_id: request_transaction_id,
                    data: request_data,
                    ..
                },
                UdpTrackerResponse {
                    transaction_id: response_transaction_id,
                    data: response_data,
                    ..
                },
            ) if request_transaction_id == response_transaction_id => {
                match (request_data, response_data) {
                    (UdpTrackerRequestData::Connect, UdpTrackerResponseData::Connect { .. })
                    | (
                        UdpTrackerRequestData::Announce { .. },
                        UdpTrackerResponseData::Announce { .. },
                    )
                    | (
                        UdpTrackerRequestData::Scrape { .. },
                        UdpTrackerResponseData::Scrape { .. },
                    ) => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }
}
