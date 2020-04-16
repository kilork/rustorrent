use crate::types::udp_tracker::UdpTrackerResponseData;

#[derive(Debug, PartialEq)]
pub(crate) struct UdpTrackerResponse {
    pub(crate) transaction_id: i32,
    pub(crate) data: UdpTrackerResponseData,
}
