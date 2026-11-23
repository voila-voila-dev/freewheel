//! The background class's payload on the wire: its exact layout, round trips, and every
//! payload it refuses (`freewheel.md`, "Background payload").

mod support;

use bonding_protocol::freewheel::*;
use support::Random;
use support::freewheel::{encode, encode_background};

#[test]
fn chunk_layout_is_kind_then_offset_then_bytes() {
    let chunk = BackgroundPayload::Chunk(RecordingChunk {
        offset: 0x0102_0304_0506_0708,
        bytes: &[0xAA, 0xBB, 0xCC],
    });
    assert_eq!(
        encode_background(&chunk),
        [0x01, 1, 2, 3, 4, 5, 6, 7, 8, 0xAA, 0xBB, 0xCC]
    );
}

#[test]
fn padding_layout_is_kind_then_zeros() {
    assert_eq!(
        encode_background(&BackgroundPayload::Padding { zeros: 3 }),
        [0, 0, 0, 0]
    );
    assert_eq!(
        encode_background(&BackgroundPayload::Padding { zeros: 0 }),
        [0]
    );
}

#[test]
fn step_5s_padding_is_padding() {
    // Capacity probing has padded with full datagrams of zeros since step 5.
    assert_eq!(
        BackgroundPayload::decode(&[0; MAX_DATA_PAYLOAD_LEN]),
        Ok(BackgroundPayload::Padding {
            zeros: MAX_DATA_PAYLOAD_LEN - 1
        })
    );
}

#[test]
fn a_full_chunk_fills_a_data_packet() {
    let bytes = [0x5A; MAX_RECORDING_CHUNK_LEN];
    let payload = encode_background(&BackgroundPayload::Chunk(RecordingChunk {
        offset: u64::MAX - 1,
        bytes: &bytes,
    }));
    assert_eq!(payload.len(), MAX_DATA_PAYLOAD_LEN);
    let packet = encode(&Packet::Data(Data {
        header: Header::default(),
        class: TrafficClass::Background,
        sequence: 1,
        sent_at_us: 2,
        payload: &payload,
    }));
    assert_eq!(packet.len(), MAX_DATAGRAM_LEN);
    let Ok(Packet::Data(data)) = Packet::decode(&packet) else {
        unreachable!("a data packet")
    };
    assert_eq!(
        BackgroundPayload::decode(data.payload),
        Ok(BackgroundPayload::Chunk(RecordingChunk {
            offset: u64::MAX - 1,
            bytes: &bytes
        }))
    );
}

#[test]
fn random_payloads_round_trip() {
    let mut random = Random::new(61);
    let mut bytes = vec![0u8; MAX_RECORDING_CHUNK_LEN];
    for _ in 0..10_000 {
        let len = 1 + random.below(MAX_RECORDING_CHUNK_LEN);
        random.fill(&mut bytes[..len]);
        let payload = if random.below(4) == 0 {
            BackgroundPayload::Padding {
                zeros: random.below(MAX_DATA_PAYLOAD_LEN),
            }
        } else {
            BackgroundPayload::Chunk(RecordingChunk {
                offset: random.next_u64(),
                bytes: &bytes[..len],
            })
        };
        let encoded = encode_background(&payload);
        assert_eq!(BackgroundPayload::decode(&encoded), Ok(payload));
    }
}

#[test]
fn refuses_what_v0_does_not_define() {
    assert_eq!(
        BackgroundPayload::decode(&[]),
        Err(BackgroundDecodeError::Empty)
    );
    assert_eq!(
        BackgroundPayload::decode(&[0x02, 0, 0]),
        Err(BackgroundDecodeError::UnknownKind { raw: 0x02 })
    );
    assert_eq!(
        BackgroundPayload::decode(&[0x00, 0, 1, 0]),
        Err(BackgroundDecodeError::PaddingNotZero)
    );
    assert_eq!(
        BackgroundPayload::decode(&[0x01, 0, 0, 0, 0, 0, 0, 0]),
        Err(BackgroundDecodeError::ChunkTooShort { len: 8 })
    );
    assert_eq!(
        BackgroundPayload::decode(&[0x01, 0, 0, 0, 0, 0, 0, 0, 7]),
        Err(BackgroundDecodeError::EmptyChunk)
    );
    assert_eq!(
        BackgroundPayload::decode(&[0; MAX_DATA_PAYLOAD_LEN + 1]),
        Err(BackgroundDecodeError::TooLong {
            len: MAX_DATA_PAYLOAD_LEN + 1
        })
    );
}

#[test]
fn will_not_write_what_it_would_refuse() {
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    assert_eq!(
        BackgroundPayload::Chunk(RecordingChunk {
            offset: 0,
            bytes: &[]
        })
        .encode(&mut out),
        Err(EncodeError::EmptyChunk)
    );
    let too_long = [0u8; MAX_RECORDING_CHUNK_LEN + 1];
    assert_eq!(
        BackgroundPayload::Chunk(RecordingChunk {
            offset: 0,
            bytes: &too_long
        })
        .encode(&mut out),
        Err(EncodeError::TooLong {
            len: MAX_DATA_PAYLOAD_LEN + 1
        })
    );
    assert_eq!(
        BackgroundPayload::Padding { zeros: 10 }.encode(&mut out[..5]),
        Err(EncodeError::BufferTooSmall { needed: 11 })
    );
}

#[test]
fn a_chunk_ends_past_its_last_byte() {
    let chunk = RecordingChunk {
        offset: 1_000,
        bytes: &[1, 2, 3],
    };
    assert_eq!(chunk.end(), 1_003);
}
