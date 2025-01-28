//! Base58 encoding and decoding functions.

const BASE58: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

#[rustfmt::skip]
const BASE58_REV: [u8; 256] = [
//  0     1     2     3     4     5     6     7     8     9     A     B     C     D     E     F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x00-0x0F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x10-0x1F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x20-0x2F
    0xFE, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x30-0x3F
    0xFE, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0xFE, 0x11, 0x12, 0x13, 0x14, 0x15, 0xFE, // 0x40-0x4F
    0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x50-0x5F
    0xFE, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0xFE, 0x2C, 0x2D, 0x2E, // 0x60-0x6F
    0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x70-0x7F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x80-0x8F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x90-0x9F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xA0-0xAF
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xB0-0xBF
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xC0-0xCF
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xD0-0xDF
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xE0-0xEF
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xF0-0xFF
];

pub fn base58_enc_step(input: [u8; 2]) -> [u8; 3] {
    let mut value = u16::from_le_bytes(input);

    let digit2 = value % 58;
    value /= 58;

    let digit1 = value % 58;
    value /= 58;

    assert!(value < 58);
    let digit0 = value;

    [
        BASE58[digit0 as usize],
        BASE58[digit1 as usize],
        BASE58[digit2 as usize],
    ]
}

pub enum Base58EncRem {
    One(u8),
    Two([u8; 2]),
}

impl AsRef<[u8]> for Base58EncRem {
    fn as_ref(&self) -> &[u8] {
        match self {
            Base58EncRem::One(a) => core::slice::from_ref(a),
            Base58EncRem::Two(a) => a,
        }
    }
}

pub fn base58_enc_rem(input: u8) -> Base58EncRem {
    let mut value = input;

    if value < 58 {
        let digit0 = value;
        Base58EncRem::One(BASE58[digit0 as usize])
    } else {
        let digit1 = value % 58;
        value /= 58;

        assert!(value < 58);
        let digit0 = value;

        Base58EncRem::Two([BASE58[digit0 as usize], BASE58[digit1 as usize]])
    }
}

pub fn base58_enc_len(input: usize) -> usize {
    let words = input / 2;
    let rem = input % 2;
    words * 3 + rem * 2
}

pub fn base58_enc_str(input: &[u8], output: &mut String) {
    let mut input = input;

    output.reserve(base58_enc_len(input.len()));

    loop {
        match *input {
            [] => break,
            [a] => {
                let rem = base58_enc_rem(a);
                let s = unsafe { core::str::from_utf8_unchecked(rem.as_ref()) };
                output.push_str(s);
                break;
            }
            [a, b, ref tail @ ..] => {
                let bytes = base58_enc_step([a, b]);
                let s = unsafe { core::str::from_utf8_unchecked(&bytes) };
                output.push_str(s);
                input = tail;
            }
        }
    }
}

pub fn base58_enc_fmt(input: &[u8], mut write: impl core::fmt::Write) -> core::fmt::Result {
    let mut input = input;

    loop {
        match *input {
            [] => break,
            [a] => {
                let rem = base58_enc_rem(a);
                let s = unsafe { core::str::from_utf8_unchecked(rem.as_ref()) };
                write.write_str(s)?;
                break;
            }
            [a, b, ref tail @ ..] => {
                let bytes = base58_enc_step([a, b]);
                let s = unsafe { core::str::from_utf8_unchecked(&bytes) };
                write.write_str(s)?;
                input = tail;
            }
        }
    }

    Ok(())
}

pub fn base58_enc_io(read: impl std::io::Read, write: impl std::io::Write) -> std::io::Result<()> {
    let mut read = read;
    let mut write = write;

    let mut buf = [0u8; 1024];
    let mut start = 0;
    let mut end = 0;

    'a: loop {
        while let [a, b, ..] = buf[start..end] {
            let bytes = base58_enc_step([a, b]);
            write.write_all(&bytes)?;
            start += 2;
        }

        if end > start {
            debug_assert_eq!(end, start + 1);
            buf[0] = buf[start];
            end = 1;
        } else {
            debug_assert_eq!(start, end);
            end = 0;
        }
        start = 0;

        while end < start + 2 {
            match read.read(&mut buf[end..]) {
                Ok(0) => break 'a,
                Ok(n) => end += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
    }

    match buf[start..end] {
        [] => {}
        [a] => {
            let rem = base58_enc_rem(a);
            write.write_all(rem.as_ref())?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, thiserror::Error)]
pub enum Base58DecodingError {
    #[error("invalid character in input")]
    InvalidCharacter,

    #[error("invalid chunk in input")]
    InvalidChunk,
}

pub fn base58_dec_step(input: [u8; 3]) -> Result<[u8; 2], Base58DecodingError> {
    let [a, b, c] = input;

    let x = BASE58_REV[a as usize];
    if x == 0xFE {
        return Err(Base58DecodingError::InvalidCharacter);
    }

    let y = BASE58_REV[b as usize];
    if y == 0xFE {
        return Err(Base58DecodingError::InvalidCharacter);
    }

    let z = BASE58_REV[c as usize];
    if z == 0xFE {
        return Err(Base58DecodingError::InvalidCharacter);
    }

    let mut result = u16::from(x);

    result = result
        .checked_mul(58)
        .ok_or(Base58DecodingError::InvalidChunk)?;

    result = result
        .checked_add(u16::from(y))
        .ok_or(Base58DecodingError::InvalidChunk)?;

    result = result
        .checked_mul(58)
        .ok_or(Base58DecodingError::InvalidChunk)?;

    result = result
        .checked_add(u16::from(z))
        .ok_or(Base58DecodingError::InvalidChunk)?;

    Ok(result.to_le_bytes())
}

pub fn base58_dec_rem_two(input: [u8; 2]) -> Result<u8, Base58DecodingError> {
    let [a, b] = input;

    let x = BASE58_REV[a as usize];
    if x == 0xFE {
        return Err(Base58DecodingError::InvalidCharacter);
    }

    let y = BASE58_REV[b as usize];
    if y == 0xFE {
        return Err(Base58DecodingError::InvalidCharacter);
    }

    let mut result = x;

    result = result
        .checked_mul(58)
        .ok_or(Base58DecodingError::InvalidChunk)?;

    result = result
        .checked_add(y)
        .ok_or(Base58DecodingError::InvalidChunk)?;

    Ok(result)
}

pub fn base58_dec_rem_one(input: u8) -> Result<u8, Base58DecodingError> {
    let a = input;

    let x = BASE58_REV[a as usize];
    if x == 0xFE {
        return Err(Base58DecodingError::InvalidCharacter);
    }

    let result = x;
    Ok(result)
}

pub fn base58_dec_len(input: usize) -> usize {
    let words = input / 3;
    let rem = input % 3;

    words * 2 + if rem == 0 { 0 } else { 1 }
}

pub fn base58_dec_slice(input: &[u8], output: &mut [u8]) -> Result<(), Base58DecodingError> {
    let mut input = input;
    let mut output = output;

    assert_eq!(output.len(), base58_dec_len(input.len()));

    loop {
        match *input {
            [] => break,
            [a] => {
                output[0] = base58_dec_rem_one(a)?;
                break;
            }
            [a, b] => {
                output[0] = base58_dec_rem_two([a, b])?;
                break;
            }
            [a, b, c, ref tail @ ..] => {
                let [x, y] = base58_dec_step([a, b, c])?;
                output[0] = x;
                output[1] = y;
                output = &mut output[2..];
                input = tail;
            }
        };
    }

    Ok(())
}

pub fn base58_dec_vec(input: &[u8], output: &mut Vec<u8>) -> Result<(), Base58DecodingError> {
    let offset = output.len();
    output.resize(offset + base58_dec_len(input.len()), 0);
    base58_dec_slice(input, &mut output[offset..])
}

pub fn base58_dec_io(read: impl std::io::Read, write: impl std::io::Write) -> std::io::Result<()> {
    let mut read = read;
    let mut write = write;

    let mut buf = [0u8; 1 << 16];
    let mut start = 0;
    let mut end = 0;

    'a: loop {
        while let [a, b, c, ..] = buf[start..end] {
            let bytes = base58_dec_step([a, b, c])
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
            write.write_all(&bytes)?;
            start += 3;
        }

        if end > start {
            buf[0] = buf[start];
            if end > start + 1 {
                debug_assert_eq!(end, start + 2);
                buf[1] = buf[start + 1];
                end = 2;
            } else {
                end = 1;
            }
        } else {
            debug_assert_eq!(start, end);
            end = 0;
        }
        start = 0;

        while end < start + 3 {
            match read.read(&mut buf[end..]) {
                Ok(0) => break 'a,
                Ok(n) => end += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
    }

    match buf[start..end] {
        [] => {}
        [a] => {
            let bytes = base58_dec_rem_one(a)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
            write.write_all(&[bytes])?;
        }
        [a, b] => {
            let bytes = base58_dec_rem_two([a, b])
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
            write.write_all(&[bytes])?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

#[cfg(test)]
mod test_base58 {
    use rand::{Rng, RngCore};

    use super::*;

    #[test]
    fn test_known_base58() {
        let data = b"Hello, world!";
        let encoded = "8i39FZ4P8A4o9i68eFa";

        let mut output = String::new();
        base58_enc_str(&data[0..2], &mut output);
        assert_eq!(output, encoded[..3]);

        base58_enc_str(&data[2..4], &mut output);
        assert_eq!(output, encoded[..6]);

        base58_enc_str(&data[4..6], &mut output);
        assert_eq!(output, encoded[..9]);

        base58_enc_str(&data[6..8], &mut output);
        assert_eq!(output, encoded[..12]);

        base58_enc_str(&data[8..10], &mut output);
        assert_eq!(output, encoded[..15]);

        base58_enc_str(&data[10..12], &mut output);
        assert_eq!(output, encoded[..18]);

        base58_enc_str(&data[12..], &mut output);
        assert_eq!(output, encoded);

        output.clear();
        base58_enc_str(data, &mut output);
        assert_eq!(output, encoded);

        let mut output = Vec::new();
        base58_dec_vec(encoded[0..3].as_bytes(), &mut output).unwrap();
        assert_eq!(&output, &data[..2]);

        base58_dec_vec(encoded[3..6].as_bytes(), &mut output).unwrap();
        assert_eq!(&output, &data[..4]);

        base58_dec_vec(encoded[6..9].as_bytes(), &mut output).unwrap();
        assert_eq!(&output, &data[..6]);

        base58_dec_vec(encoded[9..12].as_bytes(), &mut output).unwrap();
        assert_eq!(&output, &data[..8]);

        base58_dec_vec(encoded[12..15].as_bytes(), &mut output).unwrap();
        assert_eq!(&output, &data[..10]);

        base58_dec_vec(encoded[15..18].as_bytes(), &mut output).unwrap();
        assert_eq!(&output, &data[..12]);

        base58_dec_vec(encoded[18..].as_bytes(), &mut output).unwrap();
        assert_eq!(&output, data);

        output.clear();
        base58_dec_vec(encoded.as_bytes(), &mut output).unwrap();
        assert_eq!(&output, data);
    }

    #[test]
    fn test_roundtrip_base58() {
        for _ in 0..1000 {
            let size = rand::thread_rng().gen_range(42..15121);

            let mut data = Vec::new();
            data.resize(size, 0);
            rand::thread_rng().fill_bytes(&mut data);

            let mut encoded = String::new();
            base58_enc_str(&data, &mut encoded);

            let mut decoded = Vec::new();
            base58_dec_vec(encoded.as_bytes(), &mut decoded).unwrap();

            assert_eq!(data, decoded);
        }
    }

    #[test]
    fn test_roundtrip_io_base58() {
        for _ in 0..10 {
            let size = rand::thread_rng().gen_range(124141..15121523);

            let mut data = Vec::new();
            data.resize(size, 0);
            rand::thread_rng().fill_bytes(&mut data);

            let mut encoded = Vec::new();
            base58_enc_io(FussyReader { data: &data[..] }, &mut encoded).unwrap();

            let mut decoded = Vec::new();
            base58_dec_io(FussyReader { data: &encoded[..] }, &mut decoded).unwrap();

            assert_eq!(data, decoded);
        }
    }

    struct FussyReader<'a> {
        data: &'a [u8],
    }

    impl std::io::Read for FussyReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if rand::thread_rng().gen_bool(0.3) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "interrupted",
                ));
            }

            if self.data.is_empty() {
                return Ok(0);
            }

            let read = 1 + rand::thread_rng().gen_range(0..buf.len().min(self.data.len()));

            buf[..read].copy_from_slice(&self.data[..read]);
            self.data = &self.data[read..];
            Ok(read)
        }
    }
}
