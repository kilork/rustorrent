use crate::types::{BencodeBlob, BencodeValue};

use nom::*;

macro_rules! recognize_map (
    ($i:expr, $submac:ident!( $($args:tt)* ), $g:expr) => (
        {
            pub fn _unify<I, T, R, F: FnOnce(I, T) -> R>(f: F, i: I, t: T) -> R {
                f(i, t)
            }

            use nom::lib::std::result::Result::*;

            use nom::Offset;
            use nom::Slice;
            let i_ = $i.clone();
            match $submac!(i_, $($args)*) {
                Ok((i, res)) => {
                    let index = (&$i).offset(&i);
                    Ok((i, _unify($g, ($i).slice(..index), res) ))
                },
                Err(e) => Err(e)
            }
        }
    );
);

named!(
    integer_literal,
    recognize!(do_parse!(opt!(tag!("-")) >> digit >> ()))
);

named!(
    integer<i64>,
    map_res!(map_res!(integer_literal, std::str::from_utf8), |s: &str| {
        s.parse::<i64>()
    })
);

named!(
    bencode_string<BencodeValue>,
    do_parse!(len: integer >> char!(':') >> s: take!(len) >> (BencodeValue::String(s)))
);

named!(
    bencode_string_s<&str>,
    do_parse!(len: integer >> char!(':') >> s: map_res!(take!(len), std::str::from_utf8) >> (s))
);

named!(
    bencode_integer<BencodeValue>,
    delimited!(char!('i'), map!(integer, BencodeValue::Integer), char!('e'))
);

named!(
    bencode_list<BencodeValue>,
    delimited!(
        char!('l'),
        map!(many0!(parser_bencode), |x: Vec<BencodeBlob>| {
            BencodeValue::List(x)
        }),
        char!('e')
    )
);

named!(
    bencode_dictionary<BencodeValue>,
    delimited!(
        char!('d'),
        map!(
            many0!(tuple!(bencode_string_s, parser_bencode)),
            BencodeValue::Dictionary
        ),
        char!('e')
    )
);

named!(
    parser_bencode<BencodeBlob>,
    recognize_map!(
        alt!(bencode_string | bencode_integer | bencode_list | bencode_dictionary),
        |i, r| BencodeBlob {
            source: i,
            value: r
        }
    )
);

pub fn parse_bencode(bytes: &[u8]) -> BencodeBlob {
    parser_bencode(bytes).unwrap().1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BencodeBlob;

    fn blob<'a>(source: &'a [u8], value: BencodeValue<'a>) -> BencodeBlob<'a> {
        BencodeBlob { source, value }
    }

    #[test]
    fn check_bencode_string() {
        assert_eq!(
            bencode_string(b"5:UTF-8"),
            Ok((&vec![][..], BencodeValue::String(b"UTF-8")))
        );
    }

    #[test]
    fn check_bencode_integer() {
        assert_eq!(
            bencode_integer(b"i3e"),
            Ok((&vec![][..], BencodeValue::Integer(3)))
        );
    }

    #[test]
    fn check_bencode_list() {
        assert_eq!(
            bencode_list(b"l5:UTF-8i3ee"),
            Ok((
                &vec![][..],
                BencodeValue::List(vec![
                    blob(b"5:UTF-8", BencodeValue::String(b"UTF-8")),
                    blob(b"i3e", BencodeValue::Integer(3))
                ])
            ))
        );
    }
    #[test]
    fn check_bencode_dictionary() {
        assert_eq!(
            bencode_dictionary(b"d3:cow3:moo4:spam4:eggse"),
            Ok((
                &vec![][..],
                BencodeValue::Dictionary(
                    vec![
                        ("cow", blob(b"3:moo", BencodeValue::String(b"moo"))),
                        ("spam", blob(b"4:eggs", BencodeValue::String(b"eggs")))
                    ]
                    .into_iter()
                    .collect()
                )
            ))
        );

        assert_eq!(
            bencode_dictionary(b"d4:spaml1:a1:bee"),
            Ok((
                &vec![][..],
                BencodeValue::Dictionary(
                    vec![(
                        "spam",
                        blob(
                            b"l1:a1:be",
                            BencodeValue::List(vec![
                                blob(b"1:a", BencodeValue::String(b"a")),
                                blob(b"1:b", BencodeValue::String(b"b"))
                            ])
                        )
                    ),]
                    .into_iter()
                    .collect()
                )
            ))
        );
    }
}
