//! freewheel v0 on the wire: the exact layout, and every datagram it refuses.

mod support;

use bonding_protocol::freewheel::*;
use support::freewheel::encode;

const SESSION_ID: SessionId = 0x0102_0304_0506_0708;
const HEADER: Header = Header {
    session_id: SESSION_ID,
    link_id: 9,
};

#[test]
fn data_layout_is_header_then_sequence_then_send_time_then_payload() {
    let packet = Packet::Data(Data {
        header: HEADER,
        class: TrafficClass::Background,
        sequence: 0x0A0B_0C0D,
        sent_at_us: 0x1112_1314,
        payload: &[0xEE, 0xFF],
    });
    assert_eq!(
        encode(&packet),
        [
            0xF0, 0x01, 9, 0x03, 1, 2, 3, 4, 5, 6, 7, 8, 0x0A, 0x0B, 0x0C, 0x0D, 0x11, 0x12, 0x13,
            0x14, 0xEE, 0xFF,
        ]
    );
}

#[test]
fn ack_layout_is_header_then_timing_then_ranges() {
    let timing = LinkTiming {
        largest_sequence: 1,
        echoed_sent_at_us: 2,
        ack_delay_us: 3,
        received_bytes: 4,
    };
    let mut out = [0u8; MAX_DATAGRAM_LEN];
    let range = SequenceRange { first: 5, last: 6 };
    let Ok(len) = Ack::encode(HEADER, timing, &[range], &mut out) else {
        unreachable!("one range fits")
    };
    assert_eq!(
        out[..len],
        [
            0xF0, 0x02, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0,
            4, 0, 0, 0, 5, 0, 0, 0, 6,
        ]
    );
}

#[test]
fn session_packet_layouts() {
    let start = |kind: u8| [0xF0, kind, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8];
    assert_eq!(encode(&Packet::Join(HEADER)), start(0x04));
    assert_eq!(encode(&Packet::Accept(HEADER)), start(0x05));
    let keepalive = encode(&Packet::Keepalive {
        header: HEADER,
        sent_at_us: 0xDEAD_BEEF,
    });
    assert_eq!(keepalive[..HEADER_LEN], start(0x03));
    assert_eq!(keepalive[HEADER_LEN..], [0xDE, 0xAD, 0xBE, 0xEF]);
    let close = encode(&Packet::Close {
        header: HEADER,
        reason: CloseReason::TimedOut,
    });
    assert_eq!(close[..HEADER_LEN], start(0x06));
    assert_eq!(close[HEADER_LEN..], [0x02]);
}

#[test]
fn tells_freewheel_apart_from_srt_and_srtla() {
    assert!(is_freewheel(&encode(&Packet::Join(HEADER))));
    let srt_data = [0x00, 0x00, 0x00, 0x01];
    let srt_ack = [0x80, 0x02, 0x00, 0x00];
    let srtla_keepalive = [0x90, 0x00];
    let srtla_reg1 = [0x92, 0x00];
    for other in [&srt_data[..], &srt_ack, &srtla_keepalive, &srtla_reg1, &[]] {
        assert!(!is_freewheel(other));
        assert!(Packet::decode(other).is_err());
    }
    assert_eq!(Packet::decode(&srt_ack), Err(DecodeError::NotFreewheel));
}

#[test]
fn refuses_what_v0_does_not_define() {
    let join = encode(&Packet::Join(HEADER));
    let with = |at: usize, byte: u8| {
        let mut bytes = join.clone();
        bytes[at] = byte;
        Packet::decode(&bytes).err()
    };
    assert_eq!(with(0, 0xF1), Some(DecodeError::NotFreewheel));
    assert_eq!(with(1, 0x00), Some(DecodeError::UnknownKind { raw: 0 }));
    assert_eq!(with(1, 0x07), Some(DecodeError::UnknownKind { raw: 7 }));
    assert_eq!(with(3, 0x01), Some(DecodeError::ReservedNotZero));
    let mut data = join.clone();
    data[1] = PacketKind::Data.raw();
    data.extend_from_slice(&[0; 8]);
    for raw in [0x00, 0x04, 0xFF] {
        data[3] = raw;
        assert_eq!(
            Packet::decode(&data),
            Err(DecodeError::UnknownClass { raw })
        );
    }
    let mut close = encode(&Packet::Close {
        header: HEADER,
        reason: CloseReason::Done,
    });
    close[HEADER_LEN] = 0x03;
    assert_eq!(
        Packet::decode(&close),
        Err(DecodeError::UnknownCloseReason { raw: 3 })
    );
}

#[test]
fn refuses_wrong_lengths() {
    let join = encode(&Packet::Join(HEADER));
    assert_eq!(
        Packet::decode(&join[..HEADER_LEN - 1]),
        Err(DecodeError::TooShort {
            len: HEADER_LEN - 1
        })
    );
    let mut longer = join.clone();
    longer.push(0);
    assert_eq!(
        Packet::decode(&longer),
        Err(DecodeError::WrongLength {
            kind: PacketKind::Join,
            len: JOIN_LEN + 1
        })
    );
    let mut too_long = join.clone();
    too_long.resize(MAX_DATAGRAM_LEN + 1, 0);
    assert_eq!(
        Packet::decode(&too_long),
        Err(DecodeError::TooLong {
            len: MAX_DATAGRAM_LEN + 1
        })
    );
    let mut ack = join.clone();
    ack[1] = PacketKind::Ack.raw();
    ack.resize(ACK_HEADER_LEN - 1, 0);
    assert_eq!(
        Packet::decode(&ack),
        Err(DecodeError::TooShort {
            len: ACK_HEADER_LEN - 1
        })
    );
    ack.resize(ACK_HEADER_LEN + SEQUENCE_RANGE_LEN - 1, 0);
    assert_eq!(Packet::decode(&ack), Err(DecodeError::MalformedAck));
    let mut data = join;
    data[1] = PacketKind::Data.raw();
    data[3] = TrafficClass::Live.raw();
    data.resize(DATA_HEADER_LEN - 1, 0);
    assert_eq!(
        Packet::decode(&data),
        Err(DecodeError::TooShort {
            len: DATA_HEADER_LEN - 1
        })
    );
    data.push(0);
    assert!(matches!(
        Packet::decode(&data),
        Ok(Packet::Data(Data { payload: [], .. }))
    ));
}
