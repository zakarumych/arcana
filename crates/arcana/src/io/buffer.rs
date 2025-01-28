//! This module provide some IO utils.

use std::mem::MaybeUninit;

/// Abstracts over statically sized, growable, pre-initialized and uninitialized buffers.
///
/// A buffer is a contiguous memory with 4 portions:
/// - Consumed: Data that was already read from the buffer.
/// - Filled: Data that was written to the buffer.
/// - Unfilled: Unused but initialized portion of the buffer.
/// - Uninitialized: Unused and uninitialized portion of the buffer.
///
/// When unfilled portion is requested, buffer will ensure that it is initialized.
/// If capacity is reached, buffer may shift filled portion to the beginning of the buffer and grow capacity.
pub trait Buffer {
    /// Returns current buffer capacity.
    fn capacity(&self) -> usize;

    /// Returns maximum buffer capacity.
    /// Fixed buffers will return the same value as [`capacity`](Buffer::capacity).
    /// Growable buffers will return the maximum capacity they can grow to.
    fn max_capacity(&self) -> usize;

    /// Returns buffer slice filled with data.
    /// Portion filled is at the beginning of the slice.
    /// When unfilled portion is filled with data,
    /// use [`fill`](Buffer::fill) method to increase filled portion.
    fn filled(&self) -> &[u8];

    /// Mark first `len` bytes of filled portion as consumed.
    fn consume(&mut self, len: usize);

    /// Returns mutable buffer slice unfilled with data.
    /// Returned slice will be at least `min` bytes long,
    /// unless capacity is reached.
    fn unfilled(&mut self, min: usize) -> &mut [u8];

    /// Increase filled portion of the buffer.
    ///
    /// # Panics
    ///
    /// This method may panic or cause incorrect behavior
    /// if `len` is greater than `unfilled` length.
    fn fill(&mut self, len: usize);

    /// Clear the buffer, resetting filled portion to empty.
    fn clear(&mut self);
}

pub struct BorrowedBuffer<'a> {
    pub bytes: &'a mut [u8],
    pub consumed: usize,
    pub filled: usize,
}

impl<'a> BorrowedBuffer<'a> {
    pub fn new(bytes: &'a mut [u8]) -> Self {
        BorrowedBuffer {
            bytes,
            consumed: 0,
            filled: 0,
        }
    }
}

impl<'a> From<&'a mut [u8]> for BorrowedBuffer<'a> {
    fn from(bytes: &'a mut [u8]) -> Self {
        BorrowedBuffer {
            bytes,
            consumed: 0,
            filled: 0,
        }
    }
}

impl Buffer for BorrowedBuffer<'_> {
    fn capacity(&self) -> usize {
        self.bytes.len()
    }

    fn max_capacity(&self) -> usize {
        self.bytes.len()
    }

    fn consume(&mut self, len: usize) {
        debug_assert!(len <= self.filled - self.consumed);
        self.consumed += len;

        // Reuse if fully consumed.
        if self.filled == self.consumed {
            self.filled = 0;
            self.consumed = 0;
            return;
        }

        // Try perform cheap shift if possible.
        let amt = self.filled - self.consumed;
        if amt < 8 && self.consumed >= amt {
            // Shift filled portion to the beginning of the buffer.
            let (head, tail) = self.bytes.split_at_mut(self.consumed);

            head[..amt].copy_from_slice(&tail[..amt]);

            self.consumed = 0;
            self.filled = amt;
        }
    }

    fn fill(&mut self, len: usize) {
        debug_assert!(len <= self.bytes.len() - self.filled);
        self.filled += len;
    }

    fn filled(&self) -> &[u8] {
        &self.bytes[self.consumed..self.filled]
    }

    fn unfilled(&mut self, min: usize) -> &mut [u8] {
        if min > self.bytes.len() - self.filled {
            // Shift filled portion to the beginning of the buffer.
            let amt = self.filled - self.consumed;

            if self.consumed >= amt {
                let (head, tail) = self.bytes.split_at_mut(self.consumed);
                head[..amt].copy_from_slice(&tail[..amt]);
            } else {
                self.bytes.copy_within(self.consumed..self.filled, 0);
            }

            self.filled = amt;
            self.consumed = 0;
        }

        &mut self.bytes[self.filled..]
    }

    fn clear(&mut self) {
        self.consumed = 0;
        self.filled = 0;
    }
}

pub struct ArrayBuffer<const N: usize> {
    pub bytes: [u8; N],
    pub consumed: usize,
    pub filled: usize,
}

impl<const N: usize> ArrayBuffer<N> {
    pub fn new() -> Self {
        ArrayBuffer {
            bytes: [0; N],
            consumed: 0,
            filled: 0,
        }
    }
}

impl<const N: usize> Buffer for ArrayBuffer<N> {
    fn capacity(&self) -> usize {
        N
    }

    fn max_capacity(&self) -> usize {
        N
    }

    fn consume(&mut self, len: usize) {
        debug_assert!(len <= self.filled - self.consumed);
        self.consumed += len;

        // Reuse if fully consumed.
        if self.filled == self.consumed {
            self.filled = 0;
            self.consumed = 0;
            return;
        }

        // Try perform cheap shift if possible.
        if self.filled - self.consumed < 8 && self.consumed >= self.filled - self.consumed {
            // Shift filled portion to the beginning of the buffer.
            let (head, tail) = self.bytes.split_at_mut(self.consumed);

            let amt = self.filled - self.consumed;
            head[..amt].copy_from_slice(&tail[..amt]);

            self.consumed = 0;
            self.filled = amt;
        }
    }

    fn fill(&mut self, len: usize) {
        debug_assert!(len <= N - self.filled);
        self.filled += len;
    }

    fn filled(&self) -> &[u8] {
        &self.bytes[self.consumed..self.filled]
    }

    fn unfilled(&mut self, min: usize) -> &mut [u8] {
        if min > N - self.filled {
            // Shift filled portion to the beginning of the buffer.
            let amt = self.filled - self.consumed;

            if self.consumed >= amt {
                let (head, tail) = self.bytes.split_at_mut(self.consumed);
                head[..amt].copy_from_slice(&tail[..amt]);
            } else {
                self.bytes.copy_within(self.consumed..self.filled, 0);
            }

            self.filled = amt;
            self.consumed = 0;
        }

        &mut self.bytes[self.filled..]
    }

    fn clear(&mut self) {
        self.consumed = 0;
        self.filled = 0;
    }
}

pub struct GrowableBuffer {
    bytes: Box<[MaybeUninit<u8>]>,
    consumed: usize,
    filled: usize,
    initialized: usize,
}

impl GrowableBuffer {
    pub fn new() -> Self {
        GrowableBuffer {
            bytes: Box::new([]),
            consumed: 0,
            filled: 0,
            initialized: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let mut bytes = Vec::with_capacity(capacity);
        // Safety: MaybeUninit's are always initialized.
        unsafe {
            bytes.set_len(capacity);
        }
        let bytes = bytes.into_boxed_slice();
        GrowableBuffer {
            bytes,
            consumed: 0,
            filled: 0,
            initialized: 0,
        }
    }
}

impl Buffer for GrowableBuffer {
    fn capacity(&self) -> usize {
        self.bytes.len()
    }

    fn max_capacity(&self) -> usize {
        // It can grow to isize::MAX
        isize::MAX as usize
    }

    fn consume(&mut self, len: usize) {
        debug_assert!(len <= self.filled - self.consumed);
        self.consumed += len;

        // Reuse if fully consumed.
        if self.filled == self.consumed {
            self.filled = 0;
            self.consumed = 0;
            return;
        }

        // Try perform cheap shift if possible.
        if self.filled - self.consumed < 8 && self.consumed >= self.filled - self.consumed {
            // Shift filled portion to the beginning of the buffer.
            let (head, tail) = self.bytes.split_at_mut(self.consumed);

            let amt = self.filled - self.consumed;
            head[..amt].copy_from_slice(&tail[..amt]);

            self.consumed = 0;
            self.filled = amt;
        }
    }

    fn fill(&mut self, len: usize) {
        debug_assert!(len <= self.bytes.len() - self.filled);
        self.filled += len;
    }

    fn filled(&self) -> &[u8] {
        unsafe { MaybeUninit::slice_assume_init_ref(&self.bytes[self.consumed..self.filled]) }
    }

    fn unfilled(&mut self, min: usize) -> &mut [u8] {
        let mut min = min;

        // Ensure space.
        if min > self.bytes.len() - self.filled {
            let amt = self.filled - self.consumed;

            if min <= self.bytes.len() - amt {
                // Shift filled portion to the beginning of the buffer.

                if self.consumed >= amt {
                    let (head, tail) = self.bytes.split_at_mut(self.consumed);
                    head[..amt].copy_from_slice(&tail[..amt]);
                } else {
                    self.bytes.copy_within(self.consumed..self.filled, 0);
                }

                self.filled = amt;
                self.consumed = 0;
            } else {
                // Grow the buffer.
                const MIN_SIZE: usize = 16;
                let new_len = self
                    .bytes
                    .len()
                    .saturating_mul(2) // Double the size without overflow.
                    .max(amt + min) // At least min unfilled bytes.
                    .max(MIN_SIZE) // At least MIN_SIZE.
                    .min(isize::MAX as usize); // Limit to maximum capacity.

                let mut new_bytes = Vec::with_capacity(new_len);

                // Safety: MaybeUninit's are always initialized.
                unsafe {
                    new_bytes.set_len(new_len);
                }

                // Copy filled portion of the buffer.
                new_bytes[..amt].copy_from_slice(&self.bytes[self.consumed..self.filled]);
                self.bytes = new_bytes.into_boxed_slice();

                self.filled = amt;
                self.consumed = 0;

                // Only filled portion was initialized.
                self.initialized = self.filled;

                if new_len - self.filled < min {
                    // Can't return more if maximum capacity is reached.
                    min = new_len - self.filled;
                }
            }
        }

        // Ensure min bytes are initialized.
        if min > self.initialized - self.filled {
            // Initialize new portion of the buffer.
            let ptr = self.bytes[self.filled..].as_mut_ptr();
            unsafe {
                ptr.write_bytes(0, min);
            }
        }

        // Return unfilled portion.
        unsafe {
            MaybeUninit::slice_assume_init_mut(&mut self.bytes[self.filled..self.initialized])
        }
    }

    fn clear(&mut self) {
        self.consumed = 0;
        self.filled = 0;
    }
}
