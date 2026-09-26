//! One error type for writing, reading and exporting recordings.

use std::fmt;

/// Everything that can go wrong with a recording. Messages are plain English
/// and name the limit or the damaged part, so they can be shown to a player.
#[derive(Debug)]
pub enum Error {
    /// Reading or writing the file failed.
    Io(std::io::Error),
    /// The caller asked for something the format cannot hold, such as a
    /// frame beyond a limit or ticks out of order. Nothing was written.
    Invalid(String),
    /// The file's bytes do not follow the format.
    Corrupt(String),
    /// The file was written by a newer format version.
    Unsupported(String),
    /// The writer stopped after an earlier failure and accepts no more frames.
    Stopped(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "recording file error: {error}"),
            Self::Invalid(message) => write!(f, "invalid recording input: {message}"),
            Self::Corrupt(message) => write!(f, "damaged recording: {message}"),
            Self::Unsupported(message) => write!(f, "unsupported recording: {message}"),
            Self::Stopped(message) => write!(f, "recording stopped: {message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub(crate) fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}

pub(crate) fn corrupt(message: impl Into<String>) -> Error {
    Error::Corrupt(message.into())
}
