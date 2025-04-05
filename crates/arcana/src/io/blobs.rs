//! Container for blobs.
//!

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    hash::{sha256, sha256_file, Hash256},
    io::{file_eq_blob, files_eq},
};

/// Stores blobs in the FS,
/// returns ID on insertion,
/// and allows to retrieve and remove them by ID.
pub struct Blobs {
    /// Path to the blobs directory.
    path: PathBuf,
}

pub enum NewBlobsError {
    PathIsNotDir(PathBuf),
    DirectoryCreationFailed(io::Error),
    DirectoryOpenFailed(io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct BlobId {
    hash: Hash256,
    index: u64,
}

impl Blobs {
    /// Creates a new Blobs instance.
    ///
    /// # Panics
    ///
    /// Panics if the path is not a directory or if it cannot be created.
    #[must_use]
    pub fn new(path: PathBuf) -> Result<Self, NewBlobsError> {
        match path.metadata() {
            Ok(meta) => {
                if !meta.is_dir() {
                    return Err(NewBlobsError::PathIsNotDir(path));
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                if let Err(err) = fs::create_dir_all(&path) {
                    return Err(NewBlobsError::DirectoryCreationFailed(err));
                }
            }
            Err(err) => {
                return Err(NewBlobsError::DirectoryOpenFailed(err));
            }
        }

        Ok(Blobs { path })
    }

    #[must_use]
    pub fn insert(&self, blob: &[u8]) -> io::Result<BlobId> {
        insert_blob(&self.path, EitherBlob::Bytes(blob))
    }

    #[must_use]
    pub fn insert_file(&self, path: impl AsRef<Path>) -> io::Result<BlobId> {
        insert_blob(&self.path, EitherBlob::CopyFile(path.as_ref()))
    }

    #[must_use]
    pub fn insert_tmp_file(&self, path: impl AsRef<Path>) -> io::Result<BlobId> {
        insert_blob(&self.path, EitherBlob::MoveFile(path.as_ref()))
    }

    #[must_use]
    pub fn read(&self, id: BlobId) -> io::Result<Vec<u8>> {
        let BlobId { hash, index } = id;
        let path = join_path(&self.path, hash, index);
        fs::read(path)
    }
}

#[must_use]
fn join_path(path: &Path, hash: Hash256, index: u64) -> PathBuf {
    let hex = format!("{hash:x}");

    if index <= 256 {
        path.join(&hex[..index as usize])
    } else {
        path.join(format!("{hash:x}_{index}"))
    }
}

fn push_path(path: &mut PathBuf, hash: Hash256, index: u64) {
    let hex = format!("{hash:x}");

    if index <= 256 {
        path.push(&hex[..index as usize])
    } else {
        path.push(format!("{hash:x}_{index}"))
    }
}

#[derive(Clone, Copy)]
enum EitherBlob<'a> {
    Bytes(&'a [u8]),
    CopyFile(&'a Path),
    MoveFile(&'a Path),
}

#[must_use]
fn insert_blob(base: &Path, blob: EitherBlob) -> io::Result<BlobId> {
    let hash = match blob {
        EitherBlob::Bytes(bytes) => sha256(bytes),
        EitherBlob::CopyFile(path) | EitherBlob::MoveFile(path) => sha256_file(path).unwrap(),
    };

    let hex = format!("{hash:x}");

    let mut candidate = base.to_path_buf();

    for index in 8u64..u64::MAX {
        push_path(&mut candidate, hash, index);

        match candidate.metadata() {
            Ok(meta) => {
                if meta.is_file() {
                    match blob {
                        EitherBlob::Bytes(bytes) => {
                            if file_eq_blob(&candidate, bytes)? {
                                // File already exists and is equal to the blob.
                                // This is likely outcome.
                                return Ok(BlobId { hash, index });
                            }
                        }
                        EitherBlob::CopyFile(path) | EitherBlob::MoveFile(path) => {
                            if files_eq(&candidate, path)? {
                                // File already exists and is equal to the blob.
                                // This is likely outcome.
                                return Ok(BlobId { hash, index });
                            }
                        }
                    }
                }
                // Path is not a file, continue searching.
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => match blob {
                EitherBlob::Bytes(bytes) => {
                    fs::write(&candidate, bytes)?;
                    return Ok(BlobId { hash, index });
                }
                EitherBlob::CopyFile(path) => {
                    fs::copy(candidate, &path)?;
                    return Ok(BlobId { hash, index });
                }
                EitherBlob::MoveFile(path) => {
                    fs::rename(candidate, &path)?;
                    return Ok(BlobId { hash, index });
                }
            },
            Err(err) => return Err(err),
        }

        candidate.pop();
    }

    panic!("Counted to u64::MAX. Impressive, but not expected");
}
