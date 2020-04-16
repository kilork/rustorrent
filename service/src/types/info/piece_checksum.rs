use crate::SHA1_SIZE;
use serde::Deserialize;
use std::convert::{TryFrom, TryInto};

#[derive(Debug, PartialEq, Clone, Deserialize)]
pub struct PieceChecksum(pub(crate) [u8; SHA1_SIZE]);

impl TryFrom<&[u8]> for PieceChecksum {
    type Error = std::array::TryFromSliceError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        Ok(Self(value.try_into()?))
    }
}
