//! Contains error types used in the ed app.

use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

/// Error type for file open errors.
#[derive(Debug, Error, Diagnostic)]
#[error("Failed to open file at {path}")]
#[diagnostic(
    code(ed::io::file_open_error),
    help("Ensure file exists and is accessible")
)]
pub struct FileOpenError {
    pub path: PathBuf,

    #[source]
    pub source: std::io::Error,
}

/// Error type for file read errors.
#[derive(Debug, Error, Diagnostic)]
#[error("Failed to read file at {path}")]
#[diagnostic(
    code(ed::io::file_read_error),
    help("Ensure file exists and is accessible")
)]
pub struct FileReadError {
    pub path: PathBuf,

    #[source]
    pub source: std::io::Error,
}

/// Error type for file copy errors.
#[derive(Debug, Error, Diagnostic)]
#[error("Failed to copy file from {from} to {to}")]
#[diagnostic(
    code(ed::io::file_copy_error),
    help("Ensure file '{from}' exists and is accessible and path '{to}' is accessible", from = self.from.display(), to = self.to.display())
)]
pub struct FileCopyError {
    pub from: PathBuf,
    pub to: PathBuf,

    #[source]
    pub source: std::io::Error,
}
