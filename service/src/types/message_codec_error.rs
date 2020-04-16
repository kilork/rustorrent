use failure::Fail;

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
