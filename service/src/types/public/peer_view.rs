use crate::{peer::PeerState, types::public::PeerStateView};
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Serialize, Clone, Debug)]
pub struct PeerView {
    addr: SocketAddr,
    state: PeerStateView,
}

impl From<&PeerState> for PeerView {
    fn from(value: &PeerState) -> Self {
        let state = &value.state;
        Self {
            addr: value.peer.clone().into(),
            state: state.into(),
        }
    }
}
