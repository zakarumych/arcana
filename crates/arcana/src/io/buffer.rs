//! This module provide some IO utils.

use std::mem::MaybeUninit;

/// Abstracts over statically sized, growable, pre-initialized and uninitialized buffers.
pub trait Buffer {
    /// Returns maximum buffer capacity.
    /// Growable buffer should return maximum possible capacity.
    fn capacity(&self) -> usize;

    /// Returns buffer slice filled with data.
    /// Portion filled is at the beginning of the slice.
    /// When unfilled portion is filled with data,
    /// use [`fill`](Buffer::fill) method to increase filled portion.
    fn filled(&self) -> &[u8];

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

    /// Reset filled portion of the buffer to 0.
    fn reset(&mut self);
}

struct BorrowedBuffer<'a> {
    bytes: &'a mut [u8],
    filled: usize,
}

impl Buffer for BorrowedBuffer<'_> {
    fn capacity(&self) -> usize {
        self.bytes.len()
    }

    fn fill(&mut self, len: usize) {
        debug_assert!(len <= self.bytes.len() - self.filled);
        self.filled += len;
    }

    fn filled(&self) -> &[u8] {
        &self.bytes[..self.filled]
    }

    fn unfilled(&mut self, min: usize) -> &mut [u8] {
        // Min is ignored as buffer is fully initialized
        // and full capacity is returned.
        let _ = min;
        &mut self.bytes[self.filled..]
    }

    fn reset(&mut self) {
        self.filled = 0;
    }
}

struct ArrayBuffer<const N: usize> {
    bytes: [u8; N],
    filled: usize,
}

impl<const N: usize> Buffer for ArrayBuffer<N> {
    fn capacity(&self) -> usize {
        N
    }

    fn fill(&mut self, len: usize) {
        debug_assert!(len <= N - self.filled);
        self.filled += len;
    }

    fn filled(&self) -> &[u8] {
        &self.bytes[..self.filled]
    }

    fn unfilled(&mut self, min: usize) -> &mut [u8] {
        // Min is ignored as buffer is fully initialized
        // and full capacity is returned.
        let _ = min;
        &mut self.bytes[self.filled..]
    }

    fn reset(&mut self) {
        self.filled = 0;
    }
}

struct GrowableBuffer {
    bytes: Box<[MaybeUninit<u8>]>,
    filled: usize,
    initialized: usize,
}

impl Buffer for GrowableBuffer {
    fn capacity(&self) -> usize {
        // It can grow to isize::MAX
        isize::MAX as usize
    }

    fn fill(&mut self, len: usize) {
        debug_assert!(len <= self.bytes.len() - self.filled);
        self.filled += len;
    }

    fn filled(&self) -> &[u8] {
        unsafe { MaybeUninit::slice_assume_init_ref(&self.bytes[..self.filled]) }
    }

    fn unfilled(&mut self, min: usize) -> &mut [u8] {
        if min > self.bytes.len() - self.filled {
            // Grow the buffer.
            let new_len = self.bytes.len().saturating_mul(2).max(self.filled + min);
            let new_len = new_len.min(isize::MAX as usize);

            let mut new_bytes = Vec::with_capacity(new_len);
            new_bytes.extend_from_slice(&self.bytes[..self.filled]);
            self.bytes = new_bytes.into_boxed_slice();
            self.initialized = self.filled;
        }

        if min > self.initialized - self.filled {
            // Initialize new portion of the buffer.
            let ptr = self.bytes[self.filled..].as_mut_ptr();
            unsafe {
                ptr.write_bytes(0, min);
            }
        }

        unsafe {
            MaybeUninit::slice_assume_init_mut(&mut self.bytes[self.filled..self.filled + min])
        }
    }

    fn reset(&mut self) {
        self.filled = 0;
    }
}
