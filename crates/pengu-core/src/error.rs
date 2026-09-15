//! Shared error type for all PenguKit crates.

use std::io;

/// The one error type used across PenguKit.
#[derive(Debug, thiserror::Error)]
pub enum PenguError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("encoding error: {0}")]
    Utf8(#[from] std::str::Utf8Error),

    #[error("file format error: {0}")]
    Format(String),

    #[error("unsupported feature: {0}")]
    Unsupported(String),

    #[error("core native library unavailable: {0}")]
    Native(String),

    #[error("argument error: {0}")]
    Argument(String),

    #[error("unexpected end of data (needed {needed} bytes, had {available})")]
    UnexpectedEof { needed: usize, available: usize },
}

pub type Result<T> = std::result::Result<T, PenguError>;

/// Convenience for building `Format` errors.
pub fn format_err<T: Into<String>>(msg: T) -> PenguError {
    PenguError::Format(msg.into())
}

/// Convenience for building `Unsupported` errors.
pub fn unsupported_err<T: Into<String>>(msg: T) -> PenguError {
    PenguError::Unsupported(msg.into())
}