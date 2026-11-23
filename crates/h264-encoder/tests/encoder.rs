//! The encoder opens, encodes, and follows a new bitrate (skipped when built without libx264).

use h264_encoder::{Encoder, EncoderError, EncoderSettings};

const SETTINGS: EncoderSettings = EncoderSettings {
    width: 320,
    height: 240,
    fps: 30,
    keyint: 60,
    bitrate_bps: 2_000_000,
};
const FRAMES: i64 = 120;

/// A moving pattern, so the encoder has something to spend bits on.
fn frame(index: i64) -> Vec<u8> {
    let mut bytes = vec![128u8; SETTINGS.frame_bytes()];
    let width = SETTINGS.width as usize;
    let shift = usize::try_from(index).unwrap_or(0) * 3;
    for (position, byte) in bytes
        .iter_mut()
        .take(width * SETTINGS.height as usize)
        .enumerate()
    {
        let (x, y) = (position % width, position / width);
        *byte = u8::try_from((x * 7 + y * 13 + shift * 11 + (x * y) % 97) % 256).unwrap_or(0);
    }
    bytes
}

fn encoded_bytes(encoder: &mut Encoder, from: i64, frames: i64) -> usize {
    let mut out = vec![0u8; 1 << 20];
    (from..from + frames)
        .map(|index| encoder.encode(&frame(index), index, &mut out).unwrap_or(0))
        .sum()
}

#[test]
fn encodes_annex_b_and_follows_a_lower_bitrate() {
    let mut encoder = match Encoder::open(SETTINGS) {
        Err(EncoderError::Unavailable) => return,
        other => other.unwrap_or_else(|error| unreachable!("{error}")),
    };
    let mut out = vec![0u8; 1 << 20];
    let first = encoder.encode(&frame(0), 0, &mut out).unwrap_or(0);
    assert!(first > 0);
    assert_eq!(&out[..4], &[0, 0, 0, 1], "Annex B start code");
    let high = encoded_bytes(&mut encoder, 1, FRAMES);
    assert_eq!(encoder.set_bitrate_bps(300_000), Ok(()));
    let _settle = encoded_bytes(&mut encoder, 1 + FRAMES, 30);
    let low = encoded_bytes(&mut encoder, 31 + FRAMES, FRAMES);
    assert!(low * 3 < high, "high {high} bytes, low {low} bytes");
    assert_eq!(encoder.bitrate_bps(), 300_000);
}

#[test]
fn follows_a_higher_bitrate_within_a_second() {
    let Ok(mut encoder) = Encoder::open(SETTINGS) else {
        return;
    };
    let _high = encoded_bytes(&mut encoder, 0, FRAMES);
    assert_eq!(encoder.set_bitrate_bps(200_000), Ok(()));
    let _low = encoded_bytes(&mut encoder, FRAMES, 6 * 30);
    assert_eq!(encoder.set_bitrate_bps(2_000_000), Ok(()));
    // The second after the raise: at least 90 % of 2 Mbit/s (x264's ABR made 0.3 Mbit/s).
    let back_bits = encoded_bytes(&mut encoder, 7 * 30, 30) * 8;
    assert!(back_bits >= 1_800_000, "{back_bits} bits");
}

#[test]
fn refuses_odd_sizes_and_wrong_frames() {
    let odd = EncoderSettings {
        width: 321,
        ..SETTINGS
    };
    assert_eq!(
        Encoder::open(odd).err(),
        Some(EncoderError::InvalidSettings)
    );
    let Ok(mut encoder) = Encoder::open(SETTINGS) else {
        return;
    };
    let mut out = [0u8; 16];
    assert_eq!(
        encoder.encode(&[0u8; 10], 0, &mut out),
        Err(EncoderError::WrongFrameSize)
    );
}
