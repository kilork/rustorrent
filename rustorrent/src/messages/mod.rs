use super::*;

use std::sync::Arc;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::prelude::*;
use log::{debug, error, info, warn};

use crate::app::*;
use crate::errors::RustorrentError;
use crate::types::message::Message;

// mod bitfield;
// mod choke;
// mod piece;
// mod unchoke;

// pub(crate) use bitfield::message_bitfield;
// pub(crate) use choke::message_choke;
// pub(crate) use piece::message_piece;
// pub(crate) use unchoke::message_unchoke;

#[inline]
fn index_in_bitarray(index: usize) -> (usize, u8) {
    (index / 8, 128 >> (index % 8))
}

#[inline]
pub(crate) fn bit_by_index(index: usize, data: &[u8]) -> Option<(usize, u8)> {
    let (index_byte, index_bit) = index_in_bitarray(index);
    data.get(index_byte).and_then(|&v| {
        if v & index_bit == index_bit {
            Some((index_byte, index_bit))
        } else {
            None
        }
    })
}

pub(crate) fn send_message_to_peer(sender: &UnboundedSender<Message>, message: Message) {
    let mut conntx = sender.clone();
    tokio::spawn(async move {
        if let Err(err) = conntx.send(message).await {
            error!("Cannot send message: {}", err);
        }
    });
}

pub(crate) fn block_from_piece(
    piece_index: usize,
    piece_length: usize,
    block_index: usize,
    blocks_count: usize,
) -> Block {
    let is_last_block = block_index == blocks_count - 1;
    let begin = block_index as u32 * BLOCK_SIZE as u32;
    let piece = piece_index as u32;
    let length = if !is_last_block {
        BLOCK_SIZE as u32
    } else {
        piece_length as u32 - begin
    };
    Block {
        piece,
        begin,
        length,
    }
}
