use crate::types::Message;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub(crate) enum PeerMessage {
    Disconnect,
    Cancel,
    Message(Message),
    Download(usize),
    Have(usize),
    Bitfield(Vec<u8>),
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
}

impl Display for PeerMessage {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            PeerMessage::Piece {
                index,
                begin,
                block,
            } => write!(f, "Piece({}, {}, [{}])", index, begin, block.len()),
            PeerMessage::Message(message) => write!(f, "Message({})", message),
            _ => write!(f, "{:?}", self),
        }
    }
}
