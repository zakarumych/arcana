#![cfg_attr(not(feature = "std"), no_std)]

use core::fmt;

use alloc::{string::String, vec::Vec};

extern crate alloc;

pub mod base58;

/// BaseN encoding trait
pub trait Encoder {
    const ALPHABET_SIZE: usize;
    const ALPHABET: &'static [u8];

    /// Calculates the length of the encoded string based on the length of the input.
    fn encoded_len(input_len: usize) -> usize;

    /// Encodes the input bytes into a string using the this baseN encoding.
    fn encode_to_str(input: &[u8], output: &mut String);

    /// Encodes the input bytes into fmt writer using the this baseN encoding.
    fn encode_to_fmt(input: &[u8], write: impl fmt::Write) -> fmt::Result;

    /// Encodes input bytes from reader and writes the encoded output to writer.
    #[cfg(feature = "std")]
    fn encode_io(read: impl std::io::Read, write: impl std::io::Write) -> std::io::Result<()>;
}

/// BaseN decoding trait
pub trait Decoder {
    const ALPHABET_SIZE: usize;
    const ALPHABET: &'static [u8];

    #[cfg(not(feature = "std"))]
    type Error;

    #[cfg(feature = "std")]
    type Error: std::error::Error;

    /// Calculates the length of the decoded bytes based on the length of the input.
    fn decode_len(input_len: usize) -> usize;

    /// Decodes the input string into bytes using the this baseN encoding.
    fn decode_to_slice(input: &[u8], output: &mut [u8]) -> Result<(), Self::Error>;

    /// Decodes the input string into bytes using the this baseN encoding.
    fn decode_to_vec(input: &[u8], output: &mut Vec<u8>) -> Result<(), Self::Error>;

    /// Decodes input bytes from reader and writes the decoded output to writer.
    #[cfg(feature = "std")]
    fn decode_io(read: impl std::io::Read, write: impl std::io::Write) -> std::io::Result<()>;
}

#[cfg(test)]
mod tests {
    use rand::{Rng, RngCore};

    #[test]
    #[cfg(feature = "std")]
    fn test_roundtrip_io() {
        // Reads random number of bytes from input stream on each read call.
        // 30% of the time, it returns an interrupted error.
        // This is to test the error handling of the base58 decoder.
        struct FussyReader<'a> {
            data: &'a [u8],
        }

        impl std::io::Read for FussyReader<'_> {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if rand::rng().random_bool(0.3) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "interrupted",
                    ));
                }

                if self.data.is_empty() {
                    return Ok(0);
                }

                let read = 1 + rand::rng().random_range(0..buf.len().min(self.data.len()));

                buf[..read].copy_from_slice(&self.data[..read]);
                self.data = &self.data[read..];
                Ok(read)
            }
        }

        for _ in 0..10 {
            let size = rand::rng().random_range(124141..15121523);

            let mut data = Vec::new();
            data.resize(size, 0);
            rand::rng().fill_bytes(&mut data);

            let mut encoded = Vec::new();
            crate::base58::encode_io(FussyReader { data: &data[..] }, &mut encoded).unwrap();

            let mut decoded = Vec::new();
            crate::base58::decode_io(FussyReader { data: &encoded[..] }, &mut decoded).unwrap();

            assert_eq!(data, decoded);
        }
    }
}
