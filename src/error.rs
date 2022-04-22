use super::ErrorCode;
use std::convert::From;
use std::io;
use std::net;
use std::string;

#[derive(Debug)]
pub enum Error {
    AddrParse(net::AddrParseError),
    FileNotFound,
    InvalidFileName,
    InvalidMode,
    InvalidOpCode,
    InvalidPacketLength,
    Io(io::Error),
    MissingErrorMessage,
    MissingFileName,
    MissingMode,
    Timedout,
    Utf8(string::FromUtf8Error),
}

impl Error {
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Error::FileNotFound => ErrorCode::FileNotFound,
            Error::InvalidFileName
            | Error::InvalidMode
            | Error::InvalidOpCode
            | Error::InvalidPacketLength
            | Error::MissingErrorMessage
            | Error::MissingFileName
            | Error::MissingMode => ErrorCode::IllegalTftpOp,
            _ => ErrorCode::NotDefined,
        }
    }
}

impl From<net::AddrParseError> for Error {
    fn from(error: net::AddrParseError) -> Self {
        Error::AddrParse(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<string::FromUtf8Error> for Error {
    fn from(error: string::FromUtf8Error) -> Self {
        Error::Utf8(error)
    }
}
