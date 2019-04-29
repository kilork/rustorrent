use super::*;
use crate::types::message::Message;

named!(
    parser_message<Message>,
    do_parse!(
        len: be_u32
            >> m: switch!(value!(len),
                0 => value!(Message::KeepAlive) |
                1 => do_parse!(id: be_u8 >> m: switch!(value!(id),
                    0 => value!(Message::Choke) |
                    1 => value!(Message::Unchoke) |
                    2 => value!(Message::Interested) |
                    3 => value!(Message::NotInterested)
                ) >> (m)) |
                _ => do_parse!(id: be_u8 >> m: switch!(value!(id),
                    4 => cond_reduce!(len == 5, map!(be_u32, |x| Message::Have { piece_index: x})) |
                    5 => map!(take!(len - 1), |x| Message::Bitfield(x.into())) |
                    6 => cond_reduce!(len == 13, do_parse!(index: be_u32 >> begin: be_u32 >> length: be_u32 >> (Message::Request {
                        index, begin, length
                    }))) |
                    7 => cond_reduce!(len >= 9, do_parse!(index: be_u32 >> begin: be_u32 >> block: take!(len - 9) >> (Message::Piece {
                        index, begin, block: block.into()
                    }))) |
                    8 => cond_reduce!(len == 13, do_parse!(index: be_u32 >> begin: be_u32 >> length: be_u32 >> (Message::Cancel {
                        index, begin, length
                    }))) |
                    9 => cond_reduce!(len == 3, map!(be_u16, |x| Message::Port(x)))
                ) >> (m))
            )
            >> (m)
    )
);

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(buf: &[u8], message: Message) {
        assert_eq!(parser_message(buf).unwrap().1, message);
    }

    #[test]
    fn message_keepalive() {
        parse(&[0, 0, 0, 0], Message::KeepAlive);
    }

    #[test]
    fn message_choke() {
        parse(&[0, 0, 0, 1, 0], Message::Choke);
    }

    #[test]
    fn message_unchoke() {
        parse(&[0, 0, 0, 1, 1], Message::Unchoke);
    }

    #[test]
    fn message_interested() {
        parse(&[0, 0, 0, 1, 2], Message::Interested);
    }

    #[test]
    fn message_notinterested() {
        parse(&[0, 0, 0, 1, 3], Message::NotInterested);
    }

    #[test]
    fn message_have() {
        parse(
            &[0, 0, 0, 5, 4, 0, 0, 0, 10],
            Message::Have { piece_index: 10 },
        );
    }

    #[test]
    fn message_bitfield() {
        parse(&[0, 0, 0, 4, 5, 1, 2, 3], Message::Bitfield(vec![1, 2, 3]));
    }

    #[test]
    fn message_request() {
        parse(
            &[0, 0, 0, 13, 6, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3],
            Message::Request {
                index: 1,
                begin: 2,
                length: 3,
            },
        );
    }

    #[test]
    fn message_piece() {
        parse(
            &[0, 0, 0, 14, 7, 0, 0, 0, 1, 0, 0, 0, 2, 1, 2, 3, 4, 5],
            Message::Piece {
                index: 1,
                begin: 2,
                block: vec![1, 2, 3, 4, 5],
            },
        );
    }

    #[test]
    fn message_cancel() {
        parse(
            &[0, 0, 0, 13, 8, 0, 0, 0, 11, 0, 0, 0, 22, 0, 0, 0, 33],
            Message::Cancel {
                index: 11,
                begin: 22,
                length: 33,
            },
        );
    }

    #[test]
    fn message_port() {
        parse(&[0, 0, 0, 3, 9, 0, 101], Message::Port(101));
    }
}
