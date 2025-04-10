use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Error(String),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    StdError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    ParseError(nom::Err<nom::error::Error<Box<str>>>),
}
impl From<nom::Err<nom::error::Error<&str>>> for Error {
    fn from(err: nom::Err<nom::error::Error<&str>>) -> Self {
        Self::ParseError(err.map_input(|input| input.into()))
    }
}
