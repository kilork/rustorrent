use crate::parser::parser_message;

use bytes::{BufMut, BytesMut};
use failure::Fail;
use nom::Offset;
use tokio::codec::{Decoder, Encoder};

/// Messages in the protocol take the form of <length prefix><message ID><payload>. The length prefix is a four byte big-endian value. The message ID is a single decimal byte. The payload is message dependent.
#[derive(Debug, PartialEq)]
pub enum Message {
    /// keep-alive: <len=0000>
    ///
    /// The keep-alive message is a message with zero bytes, specified with the length prefix set to zero. There is no message ID and no payload. Peers may close a connection if they receive no messages (keep-alive or any other message) for a certain period of time, so a keep-alive message must be sent to maintain the connection alive if no command have been sent for a given amount of time. This amount of time is generally two minutes.
    KeepAlive,
    /// choke: <len=0001><id=0>
    ///
    /// The choke message is fixed-length and has no payload.
    Choke,
    /// unchoke: <len=0001><id=1>
    ///
    /// The unchoke message is fixed-length and has no payload.
    Unchoke,
    /// interested: <len=0001><id=2>
    ///
    /// The interested message is fixed-length and has no payload.
    Interested,
    /// not interested: <len=0001><id=3>
    ///
    /// The not interested message is fixed-length and has no payload.
    NotInterested,
    /// have: <len=0005><id=4><piece index>
    ///
    /// The have message is fixed length. The payload is the zero-based index of a piece that has just been successfully downloaded and verified via the hash.
    ///
    /// Implementer's Note: That is the strict definition, in reality some games may be played. In particular because peers are extremely unlikely to download pieces that they already have, a peer may choose not to advertise having a piece to a peer that already has that piece. At a minimum "HAVE suppression" will result in a 50% reduction in the number of HAVE messages, this translates to around a 25-35% reduction in protocol overhead. At the same time, it may be worthwhile to send a HAVE message to a peer that has that piece already since it will be useful in determining which piece is rare.
    ///
    /// A malicious peer might also choose to advertise having pieces that it knows the peer will never download. Due to this attempting to model peers using this information is a bad idea.
    Have { piece_index: u32 },
    /// bitfield: <len=0001+X><id=5><bitfield>
    ///
    /// The bitfield message may only be sent immediately after the handshaking sequence is completed, and before any other messages are sent. It is optional, and need not be sent if a client has no pieces.
    ///
    /// The bitfield message is variable length, where X is the length of the bitfield. The payload is a bitfield representing the pieces that have been successfully downloaded. The high bit in the first byte corresponds to piece index 0. Bits that are cleared indicated a missing piece, and set bits indicate a valid and available piece. Spare bits at the end are set to zero.
    ///
    /// Some clients (Deluge for example) send bitfield with missing pieces even if it has all data. Then it sends rest of pieces as have messages. They are saying this helps against ISP filtering of BitTorrent protocol. It is called lazy bitfield.
    ///
    /// A bitfield of the wrong length is considered an error. Clients should drop the connection if they receive bitfields that are not of the correct size, or if the bitfield has any of the spare bits set.
    Bitfield(Vec<u8>),
    /// request: <len=0013><id=6><index><begin><length>
    ///
    /// The request message is fixed length, and is used to request a block. The payload contains the following information:
    ///
    /// index: integer specifying the zero-based piece index
    /// begin: integer specifying the zero-based byte offset within the piece
    /// length: integer specifying the requested length.
    ///
    /// This section is under dispute! Please use the discussion page to resolve this!
    ///
    /// View #1 According to the official specification, "All current implementations use 2^15 (32KB), and close connections which request an amount greater than 2^17 (128KB)." As early as version 3 or 2004, this behavior was changed to use 2^14 (16KB) blocks. As of version 4.0 or mid-2005, the mainline disconnected on requests larger than 2^14 (16KB); and some clients have followed suit. Note that block requests are smaller than pieces (>=2^18 bytes), so multiple requests will be needed to download a whole piece.
    ///
    /// Strictly, the specification allows 2^15 (32KB) requests. The reality is near all clients will now use 2^14 (16KB) requests. Due to clients that enforce that size, it is recommended that implementations make requests of that size. Due to smaller requests resulting in higher overhead due to tracking a greater number of requests, implementers are advised against going below 2^14 (16KB).
    ///
    /// The choice of request block size limit enforcement is not nearly so clear cut. With mainline version 4 enforcing 16KB requests, most clients will use that size. At the same time 2^14 (16KB) is the semi-official (only semi because the official protocol document has not been updated) limit now, so enforcing that isn't wrong. At the same time, allowing larger requests enlarges the set of possible peers, and except on very low bandwidth connections (<256kbps) multiple blocks will be downloaded in one choke-timeperiod, thus merely enforcing the old limit causes minimal performance degradation. Due to this factor, it is recommended that only the older 2^17 (128KB) maximum size limit be enforced.
    ///
    /// View #2 This section has contained falsehoods for a large portion of the time this page has existed. This is the third time I (uau) am correcting this same section for incorrect information being added, so I won't rewrite it completely since it'll probably be broken again... Current version has at least the following errors: Mainline started using 2^14 (16384) byte requests when it was still the only client in existence; only the "official specification" still talked about the obsolete 32768 byte value which was in reality neither the default size nor maximum allowed. In version 4 the request behavior did not change, but the maximum allowed size did change to equal the default size. In latest mainline versions the max has changed to 32768 (note that this is the first appearance of 32768 for either default or max size since the first ancient versions). "Most older clients use 32KB requests" is false. Discussion of larger requests fails to take latency effects into account.
    Request { index: u32, begin: u32, length: u32 },
    /// piece: <len=0009+X><id=7><index><begin><block>
    ///
    /// The piece message is variable length, where X is the length of the block. The payload contains the following information:
    ///
    /// index: integer specifying the zero-based piece index
    /// begin: integer specifying the zero-based byte offset within the piece
    /// block: block of data, which is a subset of the piece specified by index.
    Piece {
        index: u32,
        begin: u32,
        block: Vec<u8>,
    },
    /// cancel: <len=0013><id=8><index><begin><length>
    ///
    /// The cancel message is fixed length, and is used to cancel block requests. The payload is identical to that of the "request" message. It is typically used during "End Game" (see the Algorithms section below).
    Cancel { index: u32, begin: u32, length: u32 },
    /// port: <len=0003><id=9><listen-port>
    ///
    /// The port message is sent by newer versions of the Mainline that implements a DHT tracker. The listen port is the port this peer's DHT node is listening on. This peer should be inserted in the local routing table (if DHT tracker is supported).
    Port(u16),
}

#[derive(Fail, Debug)]
pub enum MessageCodecError {
    #[fail(display = "IO Error: {}", _0)]
    IoError(std::io::Error),
    #[fail(display = "Couldn't parse incoming frame: {}", _0)]
    ParseError(String),
}

impl From<std::io::Error> for MessageCodecError {
    fn from(err: std::io::Error) -> Self {
        MessageCodecError::IoError(err)
    }
}

#[derive(Default)]
pub struct MessageCodec {}

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

        buf.split_to(consumed);

        Ok(Some(f))
    }
}

impl Encoder for MessageCodec {
    type Item = Message;
    type Error = MessageCodecError;

    fn encode(&mut self, frame: Message, buf: &mut BytesMut) -> Result<(), Self::Error> {
        match frame {
            Message::KeepAlive => {
                buf.reserve(4);
                buf.put_u32_be(0);
            }
            Message::Choke => {
                buf.reserve(5);
                buf.put_u32_be(1);
                buf.put_u8(0);
            }
            Message::Unchoke => {
                buf.reserve(5);
                buf.put_u32_be(1);
                buf.put_u8(1);
            }
            Message::Interested => {
                buf.reserve(5);
                buf.put_u32_be(1);
                buf.put_u8(2);
            }
            Message::NotInterested => {
                buf.reserve(5);
                buf.put_u32_be(1);
                buf.put_u8(3);
            }
            Message::Have { piece_index } => {
                buf.reserve(9);
                buf.put_u32_be(5);
                buf.put_u8(4);
                buf.put_u32_be(piece_index);
            }
            Message::Bitfield(bitfield) => {
                buf.reserve(5 + bitfield.len());
                buf.put_u32_be(1 + bitfield.len() as u32);
                buf.put_u8(5);
                buf.put_slice(&bitfield);
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                buf.reserve(17);
                buf.put_u32_be(13);
                buf.put_u8(6);
                buf.put_u32_be(index);
                buf.put_u32_be(begin);
                buf.put_u32_be(length);
            }
            Message::Piece {
                index,
                begin,
                block,
            } => {
                buf.reserve(13 + block.len());
                buf.put_u32_be(9 + block.len() as u32);
                buf.put_u8(7);
                buf.put_u32_be(index);
                buf.put_u32_be(begin);
                buf.put_slice(&block);
            }
            Message::Cancel {
                index,
                begin,
                length,
            } => {
                buf.reserve(17);
                buf.put_u32_be(13);
                buf.put_u8(8);
                buf.put_u32_be(index);
                buf.put_u32_be(begin);
                buf.put_u32_be(length);
            }
            Message::Port(port) => {
                buf.reserve(7);
                buf.put_u32_be(3);
                buf.put_u8(9);
                buf.put_u16_be(port);
            }
            _ => panic!("Unsupported type"),
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
