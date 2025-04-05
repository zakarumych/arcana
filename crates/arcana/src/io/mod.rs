use std::{
    fs::File,
    io::{self, Read, Seek},
    path::Path,
};

use buffer::{ArrayBuffer, Buffer, GrowableBuffer};

use self::buffer::BorrowedBuffer;

pub mod blobs;
pub mod buffer;

/// Combines [`Buffer`] and [`Read`] and implements buffering reading.
pub struct BufferRead<B, R> {
    // The cursor is used to track reading position in buffer.
    buffer: B,
    read: R,
}

impl<B, R> BufferRead<B, R> {
    pub fn from_parts(buffer: B, read: R) -> Self {
        BufferRead { buffer, read }
    }
}

impl<'a, R> BufferRead<BorrowedBuffer<'a>, R> {
    pub fn new_borrowed(buffer: &'a mut [u8], read: R) -> Self {
        BufferRead::from_parts(BorrowedBuffer::new(buffer), read)
    }
}

impl<R, const N: usize> BufferRead<ArrayBuffer<N>, R> {
    pub fn new_array(read: R) -> Self {
        BufferRead::from_parts(ArrayBuffer::new(), read)
    }
}

impl<R> BufferRead<GrowableBuffer, R> {
    pub fn new(read: R) -> Self {
        BufferRead::from_parts(GrowableBuffer::new(), read)
    }

    pub fn with_capacity(capacity: usize, read: R) -> Self {
        BufferRead::from_parts(GrowableBuffer::with_capacity(capacity), read)
    }
}

impl<B, R> BufferRead<B, R>
where
    B: Buffer,
    R: io::Read,
{
    /// Read bytes that are already in buffer.
    pub fn read_from_buffer(&mut self, buf: &mut [u8]) -> usize {
        if buf.is_empty() {
            return 0;
        }

        if self.buffer.filled().is_empty() {
            return 0;
        }

        let filled = &self.buffer.filled();
        let amt = buf.len().min(filled.len());

        if amt == 1 {
            buf[0] = filled[0];
        } else {
            buf[..amt].copy_from_slice(&filled[..amt]);
        }

        self.buffer.consume(amt);
        amt
    }

    /// Returns the contents of the internal buffer,
    /// filling it with more data from the inner reader until
    /// buffer contains at least `min` bytes,
    /// reader is exhausted or an error occurs
    /// or buffer is full.
    pub fn fill_buf(&mut self, min: usize) -> io::Result<&[u8]> {
        loop {
            if self.buffer.filled().len() >= min {
                return Ok(&self.buffer.filled());
            }

            let additional = min - self.buffer.filled().len();
            let unfilled = self.buffer.unfilled(additional);

            match self.read.read(unfilled) {
                Ok(0) => return Ok(&self.buffer.filled()),
                Ok(amt) => {
                    self.buffer.fill(amt);
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// Consumes the next `amt` bytes from the internal buffer.
    pub fn consume(&mut self, amt: usize) {
        self.buffer.consume(amt);
    }
}

impl<B, R> io::BufRead for BufferRead<B, R>
where
    B: Buffer,
    R: io::Read,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.fill_buf(1)
    }

    fn consume(&mut self, amt: usize) {
        self.consume(amt);
    }
}

impl<B, R> io::Read for BufferRead<B, R>
where
    B: Buffer,
    R: io::Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Check if buffer has unread data.
        let amt = self.read_from_buffer(buf);
        if amt > 0 {
            return Ok(amt);
        }

        // If buffer is smaller than read request, read directly,
        // but only after buffer is consumed.
        if self.buffer.capacity() < buf.len() {
            return self.read.read(buf);
        }

        let unfilled = self.buffer.unfilled(buf.len());

        match self.read.read(unfilled) {
            Ok(0) => Ok(0),
            Ok(amt) => {
                self.buffer.fill(amt);
                Ok(self.read_from_buffer(buf))
            }
            Err(e) => Err(e),
        }
    }
}

/// Compares two files byte by byte.
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
