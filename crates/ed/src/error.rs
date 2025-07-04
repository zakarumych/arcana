//! Contains error types used in the ed app.

use std::path::PathBuf;

use thiserror::Error;

/// Error type for file open errors.
#[derive(Debug, Error)]
#[error("Failed to open file at {path}")]
pub struct FileOpenError {
    pub path: PathBuf,

    #[source]
    pub source: std::io::Error,
}

/// Error type for file read errors.
#[derive(Debug, Error)]
#[error("Failed to read file at {path}")]
pub struct FileReadError {
    pub path: PathBuf,

    #[source]
    pub source: std::io::Error,
}

/// Error type for file copy errors.
#[derive(Debug, Error)]
#[error("Failed to copy file from {from} to {to}")]
pub struct FileCopyError {
    pub from: PathBuf,
    pub to: PathBuf,

    #[source]
    pub source: std::io::Error,
}
