pub mod buffer;

/// Combines [`Buffer`] and [`Read`] and implements buffering reading.
pub struct BufferRead<B, R> {
    buffer: B,
    reader: R,
}

impl<B, R> BufferRead<B, R> {
    pub fn new(buffer: B, reader: R) -> Self {
        BufferRead { buffer, reader }
    }
}

// impl<'a, R> BufferRead<BorrowedBuffer<'a>, R> {
//     pub fn new_borrowed(reader: impl Read, buffer: &mut [u8]) -> Self {
//         BufferRead::new(
//             BorrowedBuffer {
//                 bytes: buffer,
//                 filled: 0,
//             },
//             reader,
//         )
//     }
// }
