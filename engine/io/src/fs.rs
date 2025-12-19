use std::{
    fs::File,
    io::{self, Read, Seek},
    path::Path,
};

/// Compares two files for equality.
///
/// Files are different if they have different sizes.
/// Or if they have different content.
pub fn files_eq(path1: &Path, path2: &Path) -> io::Result<bool> {
    let mut file1 = File::open(path1)?;
    let mut file2 = File::open(path2)?;

    let size1 = file1.seek(io::SeekFrom::End(0))?;
    file1.rewind()?;

    let size2 = file2.seek(io::SeekFrom::End(0))?;
    file2.rewind()?;

    if size1 != size2 {
        return Ok(false);
    }

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

        let len = usize::min(len1, len2);
        if buf1[off1..][..len] != buf2[off2..][..len] {
            return Ok(false);
        }

        len1 -= len;
        len2 -= len;

        off1 += len;
        off2 += len;

        if len1 == 0 {
            off1 = 0;
        } else if off1 > 512 && buf1.len() - off1 - len1 < 512 {
            buf1.copy_within(off1..off1 + len1, 0);
            off1 = 0;
        }

        if len2 == 0 {
            off2 = 0;
        } else if off2 > 512 && buf2.len() - off2 - len2 < 512 {
            buf2.copy_within(off2..off2 + len2, 0);
            off2 = 0;
        }
    }
}

/// Compares file to blob of bytes.
pub fn file_eq_blob(path: &Path, blob: &[u8]) -> io::Result<bool> {
    let mut file1 = File::open(path)?;

    let size1 = file1.seek(io::SeekFrom::End(0))?;
    file1.rewind()?;

    if usize::try_from(size1) != Ok(blob.len()) {
        return Ok(false);
    }

    let mut buf1 = [0u8; 4096];
    let buf2 = blob;

    let mut off1 = 0;
    let mut off2 = 0;

    let mut len1 = 0;
    let mut len2 = blob.len();

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
            return Ok(len1 == 0);
        }

        let len = usize::min(len1, len2);
        if buf1[off1..][..len] != buf2[off2..][..len] {
            return Ok(false);
        }

        len1 -= len;
        len2 -= len;

        off1 += len;
        off2 += len;

        if len1 == 0 {
            off1 = 0;
        } else if off1 > 512 && buf1.len() - off1 - len1 < 512 {
            buf1.copy_within(off1..off1 + len1, 0);
            off1 = 0;
        }
    }
}
