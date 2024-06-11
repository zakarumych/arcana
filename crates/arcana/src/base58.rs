//! Base58 encoding and decoding functions.

const BASE58: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

#[rustfmt::skip]
const BASE58_REV: [u8; 256] = [
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xFE-0x1F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x20-0x3F
    0xFE, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0xFE, 0x11, 0x12, 0x13, 0x14, 0x15, 0xFE, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x40-0x5F
    0xFE, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x2B, 0xFE, 0x2C, 0x2D, 0x2E, 0x2F, 0x30, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x60-0x7F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0x80-0x9F
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xA0-0xBF
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xC0-0xDF
    0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, // 0xE0-0xFF
];

pub fn base58_enc_step(mut value: u16, output: &mut [u8; 3]) -> usize {
    let mut idx = 0;

    while value > 0 {
        let digit = value % 58;
        value /= 58;

        output[idx] = BASE58[digit as usize];
        idx += 1;
    }

    output[..idx].reverse();

    idx
}

pub fn base58_enc_fmt(input: &[u8], mut write: impl core::fmt::Write) -> core::fmt::Result {
    let mut scratch = [0u8; 3];
    let mut input = input;

    loop {
        match *input {
            [] => return Ok(()),
            [last] => {
                let len = base58_enc_step(last as u16, &mut scratch);

                let s = unsafe { core::str::from_utf8_unchecked(&scratch[..len]) };

                write.write_str(s)?;
            }
            [a, b, ref tail @ ..] => {
                let len = base58_enc_step(u16::from_le_bytes([a, b]), &mut scratch);

                let s = unsafe { core::str::from_utf8_unchecked(&scratch[..len]) };

                write.write_str(s)?;

                input = tail;
            }
        }
    }
}

pub fn base58_enc_io(input: &[u8], mut write: impl std::io::Write) -> std::io::Result<()> {
    let mut scratch = [0u8; 3];
    let mut input = input;

    loop {
        match *input {
            [] => return Ok(()),
            [last] => {
                let len = base58_enc_step(last as u16, &mut scratch);

                write.write(&scratch[..len])?;
            }
            [a, b, ref tail @ ..] => {
                let len = base58_enc_step(u16::from_le_bytes([a, b]), &mut scratch);

                write.write(&scratch[..len])?;

                input = tail;
            }
        }
    }
}

pub fn base58_enc_str(input: &[u8], output: &mut String) {
    let mut scratch = [0u8; 3];
    let mut input = input;

    loop {
        match *input {
            [] => return,
            [a] => {
                let len = base58_enc_step(a as u16, &mut scratch);

                let s = unsafe { core::str::from_utf8_unchecked(&scratch[..len]) };

                output.push_str(s);
            }
            [a, b, ref tail @ ..] => {
                let len = base58_enc_step(u16::from_le_bytes([a, b]), &mut scratch);

                let s = unsafe { core::str::from_utf8_unchecked(&scratch[..len]) };

                output.push_str(s);

                input = tail;
            }
        }
    }
}

pub enum Base58DecodingError {
    InvalidCharacter,
    InvalidChunk,
    InvalidLength,
}

pub fn base58_dec_step(input: &[u8]) -> Result<u16, Base58DecodingError> {
    let mut result = 0u16;

    for &c in input {
        let b = BASE58_REV[c as usize];
        if b == 0xFE {
            return Err(Base58DecodingError::InvalidCharacter);
        }

        result = result
            .checked_mul(58)
            .ok_or(Base58DecodingError::InvalidChunk)?;

        result
            .checked_add(u16::from(b))
            .ok_or(Base58DecodingError::InvalidChunk)?;
    }

    Ok(result)
}

pub fn base58_dec_slice(input: &[u8], output: &mut [u8]) -> Result<(), Base58DecodingError> {
    let mut input = input;
    let mut output = output;

    loop {
        let value = match *input {
            [] => return Ok(()),
            [a] => base58_dec_step(&[a])?,
            [a, b] => base58_dec_step(&[a, b])?,
            [a, b, c, ref tail @ ..] => {
                input = tail;
                base58_dec_step(&[a, b, c])?
            }
        };

        match output {
            [] => return Err(Base58DecodingError::InvalidLength),
            [a] => {
                if value > 0xFF {
                    return Err(Base58DecodingError::InvalidLength);
                }

                *a = value as u8;
                return Ok(());
            }
            [a, b, tail @ ..] => {
                let [x, y] = value.to_le_bytes();
                *a = x;
                *b = y;
                output = tail;
            }
        }
    }
}
