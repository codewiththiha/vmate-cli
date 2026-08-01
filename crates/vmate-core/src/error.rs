//! Core error types shared across modules.

use thiserror::Error;

/// Errors that can occur while parsing an `.ovpn` file.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("config file contains no 'remote' directive")]
    NoRemote,
    #[error("config file is empty")]
    Empty,
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}
