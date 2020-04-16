use failure::Fail;

#[derive(Fail, Debug)]
pub enum UdpTrackerCodecError {
    #[fail(display = "IO Error: {}", _0)]
    IoError(std::io::Error),
    #[fail(display = "Couldn't parse incoming frame: {}", _0)]
    ParseError(String),
}

impl From<std::io::Error> for UdpTrackerCodecError {
    fn from(err: std::io::Error) -> Self {
        UdpTrackerCodecError::IoError(err)
    }
}
