use super::*;

use std::sync::Arc;
use std::sync::Mutex;

use futures::prelude::*;
use futures::sync::mpsc::{channel, Receiver, Sender};
use log::{debug, error, info, warn};

use crate::app::{TorrentPeer, TorrentPeerState, TorrentProcess};
use crate::errors::{RustorrentError, TryFromBencode};
use crate::types::message::Message;

mod bitfield;
mod unchoke;

pub(crate) use bitfield::message_bitfield;
pub(crate) use unchoke::message_unchoke;

#[inline]
fn index_in_bitarray(index: usize) -> (usize, u8) {
    (index / 8, 128 >> (index % 8))
}

fn send_message_to_peer(sender: &Sender<Message>, message: Message) {
    let conntx = sender.clone();
    tokio::spawn(
        conntx
            .send(message)
            .map(|_| ())
            .map_err(|err| error!("Cannot send message: {}", err)),
    );
}
