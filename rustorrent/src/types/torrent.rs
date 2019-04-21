use super::*;

use std::convert::TryInto;

#[derive(Debug)]
pub struct Torrent<'a> {
    pub raw: &'a [u8],
    pub announce: &'a str,
    pub announce_list: Option<Vec<Vec<&'a str>>>,
    pub creation_date: Option<i64>,
}

impl<'a> TryFrom<BencodeBlob<'a>> for Torrent<'a> {
    type Error = TryFromBencode;

    fn try_from(value: BencodeBlob<'a>) -> Result<Self, Self::Error> {
        let dictionary: Vec<_> = value.value.try_into()?;

        let mut announce = None;
        let mut announce_list = None;
        let mut creation_date = None;

        for (key, value) in dictionary {
            match key {
                "announce" => announce = Some(value.value.try_into()?),
                "announce-list" => {
                    let value_list: Vec<BencodeBlob> = value.value.try_into()?;
                    let value_list_list: Vec<Vec<&str>> = value_list
                        .into_iter()
                        .map(|l| l.value.try_into().unwrap())
                        .map(|l: Vec<BencodeBlob>| {
                            l.into_iter().map(|k| k.value.try_into().unwrap()).collect()
                        })
                        .collect();
                    announce_list = Some(value_list_list);
                }
                "creation date" => creation_date = Some(value.value.try_into()?),
                _ => (),
            }
        }

        Ok(Torrent {
            raw: value.source,
            announce: announce.unwrap(),
            announce_list,
            creation_date,
        })
    }
}
