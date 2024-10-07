use std::io::*;

use buffer::{ArrayBuffer, Buffer, GrowableBuffer};

use self::buffer::BorrowedBuffer;

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
    R: Read,
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
    pub fn fill_buf(&mut self, min: usize) -> Result<&[u8]> {
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
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// Consumes the next `amt` bytes from the internal buffer.
    pub fn consume(&mut self, amt: usize) {
        self.buffer.consume(amt);
    }
}

impl<B, R> BufRead for BufferRead<B, R>
where
    B: Buffer,
    R: Read,
{
    fn fill_buf(&mut self) -> Result<&[u8]> {
        self.fill_buf(1)
    }

    fn consume(&mut self, amt: usize) {
        self.consume(amt);
    }
}

impl<B, R> Read for BufferRead<B, R>
where
    B: Buffer,
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
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
