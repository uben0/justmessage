use serde::{de, ser};
use std::fmt::{self, Display};

pub type Result<T> = std::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(_: std::io::Error) -> Self {
        Self::Io
    }
}

#[derive(Debug, Clone)]
pub enum Error {
    Io,
    Message(String),
    InvalidUtf8,
    Eof,
    InvalidEscape(char),
    NumberOverflow,
    ExpectedChar {
        one_of: Vec<char>,
        found: char,
        row: usize,
        col: usize,
    },
    ExpectedIdentifier,
    TrailingCharacters,
}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error::Message(msg.to_string())
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Io => write!(formatter, "io error"),
            Error::InvalidUtf8 => write!(formatter, "invalid utf8"),
            Error::ExpectedChar {
                one_of,
                found,
                row,
                col,
            } => {
                write!(
                    formatter,
                    "expecting one of {:?} but found {:?} at {}:{}",
                    one_of, found, col, row
                )
            }
            Error::TrailingCharacters => {
                write!(formatter, "expecting end of input")
            }
            Error::ExpectedIdentifier => write!(formatter, "expecting an identifier"),
            Error::Eof => write!(formatter, "unexpected end of input"),
            Error::InvalidEscape(c) => write!(formatter, "invalid escape {:?}", c),
            Error::Message(msg) => write!(formatter, "{}", msg),
            Error::NumberOverflow => write!(formatter, "number too big"),
        }
    }
}

impl std::error::Error for Error {}
