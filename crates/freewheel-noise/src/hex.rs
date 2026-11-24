//! Keys as text: 64 lowercase hexadecimal digits.

use bonding_protocol::freewheel::PUBLIC_KEY_LEN;

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// `bytes` as lowercase hexadecimal.
pub(crate) fn encode(bytes: &[u8; PUBLIC_KEY_LEN]) -> String {
    let mut text = String::with_capacity(2 * PUBLIC_KEY_LEN);
    for byte in bytes {
        for nibble in [byte >> 4, byte & 0x0F] {
            text.push(char::from(DIGITS[usize::from(nibble)]));
        }
    }
    text
}

/// 64 hexadecimal digits (either case) as 32 bytes; `None` for anything else.
pub(crate) fn decode(text: &str) -> Option<[u8; PUBLIC_KEY_LEN]> {
    let digits = text.as_bytes();
    if digits.len() != 2 * PUBLIC_KEY_LEN {
        return None;
    }
    let mut bytes = [0u8; PUBLIC_KEY_LEN];
    for (byte, pair) in bytes.iter_mut().zip(digits.chunks_exact(2)) {
        let (high, low) = (value(*pair.first()?)?, value(*pair.get(1)?)?);
        *byte = high << 4 | low;
    }
    Some(bytes)
}

const fn value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}
