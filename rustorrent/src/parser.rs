use crate::types::Bencode;

use nom::*;

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
    bencode_string<Bencode>,
    do_parse!(len: integer >> char!(':') >> s: take!(len) >> (Bencode::String(s.into())))
);

named!(
    bencode_string_s<String>,
    do_parse!(
        len: integer >> char!(':') >> s: map_res!(take!(len), std::str::from_utf8) >> (s.into())
    )
);

named!(
    bencode_integer<Bencode>,
    delimited!(
        char!('i'),
        map!(integer, |x: i64| Bencode::Integer(x)),
        char!('e')
    )
);

named!(
    bencode_list<Bencode>,
    delimited!(
        char!('l'),
        map!(many0!(parser_bencode), |x: Vec<Bencode>| Bencode::List(x)),
        char!('e')
    )
);

named!(
    bencode_dictionary<Bencode>,
    delimited!(
        char!('d'),
        map!(
            many0!(tuple!(bencode_string_s, parser_bencode)),
            |x: Vec<(String, Bencode)>| Bencode::Dictionary(x.into_iter().collect())
        ),
        char!('e')
    )
);

named!(
    parser_bencode<Bencode>,
    alt!(bencode_string | bencode_integer | bencode_list | bencode_dictionary)
);

pub fn parse_bencode(bytes: &[u8]) -> Bencode {
    parser_bencode(bytes).unwrap().1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Bencode;

    #[test]
    fn check_bencode_string() {
        assert_eq!(
            bencode_string(b"5:UTF-8"),
            Ok((&vec![][..], Bencode::String(b"UTF-8".to_vec())))
        );
    }

    #[test]
    fn check_bencode_integer() {
        assert_eq!(
            bencode_integer(b"i3e"),
            Ok((&vec![][..], Bencode::Integer(3)))
        );
    }

    #[test]
    fn check_bencode_list() {
        assert_eq!(
            bencode_list(b"l5:UTF-8i3ee"),
            Ok((
                &vec![][..],
                Bencode::List(vec![
                    Bencode::String(b"UTF-8".to_vec()),
                    Bencode::Integer(3)
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
                Bencode::Dictionary(
                    vec![
                        ("cow".into(), Bencode::String(b"moo".to_vec())),
                        ("spam".into(), Bencode::String(b"eggs".to_vec()))
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
                Bencode::Dictionary(
                    vec![(
                        "spam".into(),
                        Bencode::List(vec![
                            Bencode::String(b"a".to_vec()),
                            Bencode::String(b"b".to_vec())
                        ])
                    ),]
                    .into_iter()
                    .collect()
                )
            ))
        );
    }
}
