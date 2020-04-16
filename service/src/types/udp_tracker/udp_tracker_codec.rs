use crate::parser::parser_udp_tracker;
use crate::types::udp_tracker::{
    UdpTrackerCodecError, UdpTrackerRequest, UdpTrackerRequestData, UdpTrackerResponse,
};
use bytes::{Buf, BufMut, BytesMut};
use nom::Offset;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Default)]
pub(crate) struct UdpTrackerCodec;

impl Decoder for UdpTrackerCodec {
    type Item = UdpTrackerResponse;
    type Error = UdpTrackerCodecError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let (consumed, f) = match parser_udp_tracker(buf) {
            Err(e) => {
                if e.is_incomplete() {
                    return Ok(None);
                } else {
                    return Err(UdpTrackerCodecError::ParseError(format!("{:?}", e)));
                }
            }
            Ok((i, frame)) => (buf.offset(i), frame),
        };

        buf.advance(consumed);

        Ok(Some(f))
    }
}

impl Encoder<UdpTrackerRequest> for UdpTrackerCodec {
    type Error = UdpTrackerCodecError;

    fn encode(&mut self, frame: UdpTrackerRequest, buf: &mut BytesMut) -> Result<(), Self::Error> {
        let UdpTrackerRequest {
            connection_id,
            transaction_id,
            data,
            ..
        } = frame;
        match data {
            UdpTrackerRequestData::Connect => {
                buf.reserve(16);
                buf.put_i64(connection_id);
                buf.put_i32(0);
                buf.put_i32(transaction_id);
            }
            UdpTrackerRequestData::Announce {
                info_hash,
                peer_id,
                downloaded,
                left,
                uploaded,
                event,
                ip,
                key,
                num_want,
                port,
                extensions,
            } => {
                buf.reserve(16 + 20 + 20 + 8 + 8 + 8 + 4 + 4 + 4 + 4 + 2 + 2);
                buf.put_i64(connection_id);
                buf.put_i32(1);
                buf.put_i32(transaction_id);
                buf.put(info_hash.as_ref());
                buf.put(peer_id.as_ref());
                buf.put_i64(downloaded);
                buf.put_i64(left);
                buf.put_i64(uploaded);
                buf.put_i32(event);
                buf.put_u32(ip);
                buf.put_u32(key);
                buf.put_i32(num_want);
                buf.put_u16(port);
                buf.put_u16(extensions);
            }
            UdpTrackerRequestData::Scrape { info_hashes } => {
                buf.reserve(16 + 20 * info_hashes.len());
                buf.put_i64(connection_id);
                buf.put_i32(2);
                buf.put_i32(transaction_id);
                for info_hash in info_hashes {
                    buf.put(info_hash.as_ref());
                }
            }
        }

        Ok(())
    }
}
