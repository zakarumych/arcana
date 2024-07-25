use std::{
    cmp::min,
    fs::metadata,
    io::Read,
    path::{Path, PathBuf},
};

pub(crate) const PREFIX_STARTING_LEN: usize = 8;

/// Tries to find non-occupied path in given directory
/// using hex-string representation of file hash.
///
/// This function iterates over all possible prefixes of given hex-string
/// and then over all possible suffixes.
///
/// Calls provided closure with each path candidate.
/// When closure returns `Ok(None)` then next candidate is tried.
/// When closure returns `Ok(Some(ok))` then this value is returned from this function.
/// When closure returns `Err(err)` then this error is returned from this function.
pub(crate) fn with_path_candidates<T, E>(
    hex: &str,
    base: &Path,
    mut f: impl FnMut(PathBuf, u64) -> Result<Option<T>, E>,
) -> Result<T, E> {
    use std::fmt::Write;

    for len in PREFIX_STARTING_LEN..=hex.len() {
        let path = base.join(&hex[..len]);

        match f(path, len as u64) {
            Ok(None) => {}
            Ok(Some(ok)) => return Ok(ok),
            Err(err) => return Err(err),
        }
    }

    // Rarely needed.
    let mut name = hex.to_owned();

    for suffix in 0usize.. {
        name.truncate(hex.len());
        write!(name, ":{}", suffix).unwrap();

        let path = base.join(&name);

        match f(path, (hex.len() + suffix) as u64) {
            Ok(None) => {}
            Ok(Some(ok)) => return Ok(ok),
            Err(err) => return Err(err),
        }
    }

    unreachable!()
}

/// Stores copy of the content in the base directory.
/// Returns path to the file with stored data.
///
/// This function uses hex-string representation of content hash
/// for the new file name.
/// It walks over prefix length starting with [`PREFIX_STARTING_LEN`]
/// and then over suffixes from 0 to i64::MAX.
/// For each name candidate it checks if file with this name exists
/// and if it has the same content.
/// If identical file is found then its path is returned.
/// If non-occupied path is found then data is written to new file at the path
/// and the path is returned.
pub(crate) fn store_data_with_content_address(
    hex: &str,
    data: &[u8],
    base: &Path,
) -> std::io::Result<(PathBuf, u64)> {
    with_path_candidates(hex, base, move |path, len| match path.metadata() {
        Err(_) => {
            std::fs::write(&path, data)?;
            Ok(Some((path, len)))
        }
        Ok(metadata) if metadata.is_file() && metadata.len() == data.len() as u64 => {
            let mut file = std::fs::File::open(&path)?;
            let mut buf = [0u8; 4096];
            let mut offset = 0;

            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                if n > data.len() - offset {
                    return Ok(None);
                }
                if buf[..n] != data[offset..][..n] {
                    return Ok(None);
                }
                offset += n;
            }

            std::fs::write(&path, data)?;
            Ok(Some((path, len)))
        }
        Ok(_) => Ok(None),
    })
}

/// Moves file to the base directory.
/// Returns path to the new file.
///
/// This function uses hex-string representation of content hash
/// for the new file name.
/// It walks over prefix length starting with [`PREFIX_STARTING_LEN`]
/// and then over suffixes from 0 to i64::MAX.
/// For each name candidate it checks if file with this name exists
/// and if it has the same content.
/// If identical file is found then its path is returned.
/// If non-occupied path is found then file is moved to the path
/// and the path is returned.
pub(crate) fn move_file_with_content_address(
    hex: &str,
    file: &Path,
    base: &Path,
) -> std::io::Result<(PathBuf, u64)> {
    let file_len = metadata(file)?.len();

    with_path_candidates(hex, base, move |path, len| match path.metadata() {
        Err(_) => {
            std::fs::rename(&file, &path)?;
            Ok(Some((path, len)))
        }
        Ok(metadata) => {
            if metadata.len() == file_len {
                if files_eq(file, &path)? {
                    Ok(Some((path, len)))
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            }
        }
    })
}

fn files_eq(path1: &Path, path2: &Path) -> std::io::Result<bool> {
    let mut file1 = std::fs::File::open(path1)?;
    let mut file2 = std::fs::File::open(path2)?;

    let mut buf1 = [0u8; 4096];
    let mut buf2 = [0u8; 4096];

    let mut off1 = 0;
    let mut off2 = 0;

    let mut len1 = 0;
    let mut len2 = 0;

    loop {
        if len1 == 0 {
            let n = file1.read(&mut buf1[off1 + len1..])?;
            if n == 0 {
                if len1 < len2 {
                    return Ok(false);
                }
            }
            len1 += n;
        }

        if len2 == 0 {
            let n = file2.read(&mut buf2[off2 + len2..])?;
            if n == 0 {
                if len1 == 0 {
                    return Ok(true);
                }
                return Ok(false);
            }
            len1 += n;
        }

        let len = min(len1, len2);
        if buf1[off1..][..len] != buf2[off2..][..len] {
            return Ok(false);
        }

        len1 -= len;
        len2 -= len;

        off1 += len;
        off2 += len;

        if len1 == 0 {
            off1 = 0;
        } else if len1 < 512 && buf1.len() - off1 - len1 < 512 {
            buf1.copy_within(off1..off1 + len1, 0);
            off1 = 0;
        }

        if len2 == 0 {
            off2 = 0;
        } else if len2 < 512 && buf2.len() - off2 - len2 < 512 {
            buf2.copy_within(off2..off2 + len2, 0);
            off2 = 0;
        }
    }
}
