use crate::{
    parser::parser_message,
    types::{Message, MessageCodecError},
};
use bytes::{Buf, BufMut, BytesMut};
use nom::Offset;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Default)]
pub struct MessageCodec;

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = MessageCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (consumed, f) = match parser_message(buf) {
            Err(e) => {
                if e.is_incomplete() {
                    return Ok(None);
                } else {
                    return Err(MessageCodecError::ParseError(format!("{:?}", e)));
                }
            }
            Ok((i, frame)) => (buf.offset(i), frame),
        };

        buf.advance(consumed);

        Ok(Some(f))
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = MessageCodecError;

    fn encode(&mut self, frame: Message, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match frame {
            Message::KeepAlive => {
                buf.reserve(4);
                buf.put_u32(0);
            }
            Message::Choke => {
                buf.reserve(5);
                buf.put_u32(1);
                buf.put_u8(0);
            }
            Message::Unchoke => {
                buf.reserve(5);
                buf.put_u32(1);
                buf.put_u8(1);
            }
            Message::Interested => {
                buf.reserve(5);
                buf.put_u32(1);
                buf.put_u8(2);
            }
            Message::NotInterested => {
                buf.reserve(5);
                buf.put_u32(1);
                buf.put_u8(3);
            }
            Message::Have { piece_index } => {
                buf.reserve(9);
                buf.put_u32(5);
                buf.put_u8(4);
                buf.put_u32(piece_index);
            }
            Message::Bitfield(bitfield) => {
                buf.reserve(5 + bitfield.len());
                buf.put_u32(1 + bitfield.len() as u32);
                buf.put_u8(5);
                buf.put_slice(&bitfield);
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                buf.reserve(17);
                buf.put_u32(13);
                buf.put_u8(6);
                buf.put_u32(index);
                buf.put_u32(begin);
                buf.put_u32(length);
            }
            Message::Piece {
                index,
                begin,
                block,
            } => {
                buf.reserve(13 + block.len());
                buf.put_u32(9 + block.len() as u32);
                buf.put_u8(7);
                buf.put_u32(index);
                buf.put_u32(begin);
                buf.put_slice(&block);
            }
            Message::Cancel {
                index,
                begin,
                length,
            } => {
                buf.reserve(17);
                buf.put_u32(13);
                buf.put_u8(8);
                buf.put_u32(index);
                buf.put_u32(begin);
                buf.put_u32(length);
            }
            Message::Port(port) => {
                buf.reserve(7);
                buf.put_u32(3);
                buf.put_u8(9);
                buf.put_u16(port);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn encode_message(expected: &[u8], message: Message) {
        let mut buf = BytesMut::new();

        let mut message_codec = MessageCodec {};
        message_codec.encode(message, &mut buf).unwrap();

        assert_eq!(expected, &buf[..]);
    }

    #[test]
    fn encode_keep_alive() {
        encode_message(&[0, 0, 0, 0], Message::KeepAlive);
    }

    #[test]
    fn encode_choke() {
        encode_message(&[0, 0, 0, 1, 0], Message::Choke);
    }

    #[test]
    fn encode_unchoke() {
        encode_message(&[0, 0, 0, 1, 1], Message::Unchoke);
    }

    #[test]
    fn encode_interested() {
        encode_message(&[0, 0, 0, 1, 2], Message::Interested);
    }

    #[test]
    fn encode_notinterested() {
        encode_message(&[0, 0, 0, 1, 3], Message::NotInterested);
    }

    #[test]
    fn encode_have() {
        encode_message(
            &[0, 0, 0, 5, 4, 0, 0, 0, 10],
            Message::Have { piece_index: 10 },
        );
    }

    #[test]
    fn encode_bitfield() {
        encode_message(&[0, 0, 0, 4, 5, 1, 2, 3], Message::Bitfield(vec![1, 2, 3]));
    }

    #[test]
    fn encode_request() {
        encode_message(
            &[0, 0, 0, 13, 6, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3],
            Message::Request {
                index: 1,
                begin: 2,
                length: 3,
            },
        );
    }

    #[test]
    fn encode_piece() {
        encode_message(
            &[0, 0, 0, 14, 7, 0, 0, 0, 1, 0, 0, 0, 2, 1, 2, 3, 4, 5],
            Message::Piece {
                index: 1,
                begin: 2,
                block: vec![1, 2, 3, 4, 5],
            },
        );
    }

    #[test]
    fn encode_cancel() {
        encode_message(
            &[0, 0, 0, 13, 8, 0, 0, 0, 11, 0, 0, 0, 22, 0, 0, 0, 33],
            Message::Cancel {
                index: 11,
                begin: 22,
                length: 33,
            },
        );
    }

    #[test]
    fn encode_port() {
        encode_message(&[0, 0, 0, 3, 9, 0, 101], Message::Port(101));
    }
}
