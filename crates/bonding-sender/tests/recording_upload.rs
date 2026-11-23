//! The recording upload's state on scripted ACKs (`RecordingUpload`): what it sends, what it
//! finds delivered or lost, and that it gets the whole recording through an outage of every link.

use std::path::PathBuf;

use bonding_protocol::freewheel::SequenceRange;
use bonding_sender::freewheel::{ChunkToSend, RecordingSettings, RecordingUpload};

const CHUNK: u64 = 1_000;
const MS: u64 = 1_000;

fn settings() -> RecordingSettings {
    RecordingSettings {
        chunk_bytes: 1_000,
        window_bytes: 100 * CHUNK,
        in_flight_chunks: 50,
        ..RecordingSettings::new(PathBuf::from("/unused"))
    }
}

/// An upload and the sequences it sent, as a sender would number them.
struct Script {
    upload: RecordingUpload,
    next_sequence: u32,
    now_us: u64,
}

impl Script {
    fn new(settings: &RecordingSettings) -> Self {
        Self {
            upload: RecordingUpload::new(settings),
            next_sequence: 0,
            now_us: 0,
        }
    }

    fn starting_at(sequence: u32) -> Self {
        Self {
            next_sequence: sequence,
            ..Self::new(&settings())
        }
    }

    /// The recording is `bytes` long, `later_us` from now.
    fn grows_to(&mut self, bytes: u64, later_us: u64) -> bool {
        self.now_us += later_us;
        self.upload.on_recording_bytes(self.now_us, bytes)
    }

    /// Sends up to `count` chunks on `link`: (sequence, chunk) of each.
    fn send(&mut self, link: u8, count: usize) -> Vec<(u32, ChunkToSend)> {
        let mut sent = Vec::new();
        while sent.len() < count {
            let Some(chunk) = self.upload.next_chunk(self.now_us) else {
                break;
            };
            let sequence = self.next_sequence;
            self.next_sequence = self.next_sequence.wrapping_add(1);
            self.upload.on_sent(chunk, sequence, link, self.now_us);
            sent.push((sequence, chunk));
        }
        sent
    }

    /// A live packet goes out on some link: it takes a sequence.
    fn live(&mut self) -> u32 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        sequence
    }

    /// An ACK of `link` up to `largest`, with `ranges` received.
    fn ack(&mut self, link: u8, largest: u32, ranges: &[(u32, u32)]) {
        self.upload.on_acknowledged(
            ranges
                .iter()
                .map(|&(first, last)| SequenceRange { first, last }),
        );
        self.upload.on_link_acknowledged(link, largest);
    }
}

fn offsets(sent: &[(u32, ChunkToSend)]) -> Vec<u64> {
    sent.iter().map(|(_, chunk)| chunk.offset).collect()
}

#[test]
fn whole_chunks_while_it_grows_then_the_tail_once_it_ended() {
    let mut script = Script::new(&settings());
    assert!(!script.grows_to(2_500, 0));
    let sent = script.send(0, 10);
    assert_eq!(
        offsets(&sent),
        [0, 1_000],
        "the tail waits: it may still grow"
    );
    assert!(!script.grows_to(2_500, 1_000 * MS));
    assert!(script.send(0, 10).is_empty());
    assert!(
        script.grows_to(2_500, 1_000 * MS),
        "2 s without growing: ended"
    );
    let tail = script.send(0, 10);
    assert_eq!(
        tail.iter().map(|(_, chunk)| *chunk).collect::<Vec<_>>(),
        [ChunkToSend {
            offset: 2_000,
            len: 500
        }]
    );
    let (first, last) = (sent[0].0, tail[0].0);
    script.ack(0, last, &[(first, last)]);
    assert!(script.upload.complete());
    assert_eq!(script.upload.delivered_bytes(), 2_500);
}

#[test]
fn a_chunk_a_later_ack_of_its_link_skips_is_lost_and_goes_first() {
    let mut script = Script::new(&settings());
    script.grows_to(10 * CHUNK, 0);
    let sent = script.send(0, 4);
    // 0, 2 and 3 arrive; 1 does not.
    script.ack(
        0,
        sent[3].0,
        &[(sent[0].0, sent[0].0), (sent[2].0, sent[3].0)],
    );
    assert_eq!(script.upload.lost_chunks(), 1);
    assert_eq!(offsets(&script.send(0, 2)), [CHUNK, 4 * CHUNK]);
    assert_eq!(script.upload.delivered_bytes(), 3 * CHUNK);
}

#[test]
fn another_links_ack_says_nothing_of_its_chunks() {
    let mut script = Script::new(&settings());
    script.grows_to(10 * CHUNK, 0);
    let on_link_0 = script.send(0, 2);
    let on_link_1 = script.send(1, 2);
    script.ack(1, on_link_1[1].0, &[(on_link_1[0].0, on_link_1[1].0)]);
    assert_eq!(
        script.upload.lost_chunks(),
        0,
        "{on_link_0:?} are still on their way"
    );
    script.ack(0, on_link_0[1].0, &[(on_link_0[0].0, on_link_0[1].0)]);
    assert_eq!(script.upload.delivered_bytes(), 4 * CHUNK);
}

#[test]
fn every_link_down_then_back_delivers_the_whole_recording_once() {
    let mut script = Script::new(&settings());
    script.grows_to(30 * CHUNK, 0);
    let before = [script.send(0, 5), script.send(1, 5)];
    for (link, sent) in (0..).zip(&before) {
        let (first, last) = (sent[0].0, sent[4].0);
        script.ack(link, last, &[(first, first + 2)]);
    }
    // Outage: what was in flight is gone, both links stall; the recording keeps growing.
    let lost_on_the_way = [script.send(0, 3), script.send(1, 3)];
    assert_eq!(lost_on_the_way.iter().flatten().count(), 6);
    script.upload.on_link_lost(0);
    script.upload.on_link_lost(1);
    assert_eq!(script.upload.lost_chunks(), 4 + 6);
    script.grows_to(40 * CHUNK, 6_000 * MS);
    // Back: everything not delivered goes again, lowest first, then the rest.
    let mut delivered = 6 * CHUNK;
    let mut rounds = 0;
    while delivered < 40 * CHUNK && rounds < 100 {
        rounds += 1;
        for link in 0..2 {
            let sent = script.send(link, 7);
            if let (Some(first), Some(last)) = (sent.first(), sent.last()) {
                script.ack(link, last.0, &[(first.0, last.0)]);
            }
        }
        delivered = script.upload.delivered_bytes();
    }
    assert_eq!(delivered, 40 * CHUNK);
    assert_eq!(
        script.upload.sent_chunks() - script.upload.lost_chunks(),
        40,
        "each chunk delivered exactly once"
    );
}

#[test]
fn a_chunk_unacknowledged_past_the_timeout_is_lost() {
    let mut script = Script::new(&settings());
    script.grows_to(5 * CHUNK, 0);
    let sent = script.send(0, 2);
    script.now_us += settings().loss_timeout_us - 1;
    script.upload.expire(script.now_us);
    assert_eq!(script.upload.lost_chunks(), 0);
    script.now_us += 1;
    script.upload.expire(script.now_us);
    assert_eq!(script.upload.lost_chunks(), 2, "{sent:?}");
    assert_eq!(offsets(&script.send(0, 1)), [0]);
}

#[test]
fn a_late_delivery_after_a_loss_counts_once_and_is_not_resent() {
    let mut script = Script::new(&settings());
    script.grows_to(5 * CHUNK, 0);
    let sent = script.send(0, 1);
    let later = script.live();
    script.ack(0, later, &[]);
    assert_eq!(script.upload.lost_chunks(), 1);
    // The resend goes, then the first copy turns out to have arrived.
    let resent = script.send(0, 1);
    assert_eq!(offsets(&resent), [0]);
    script.ack(0, resent[0].0, &[(sent[0].0, resent[0].0)]);
    assert_eq!(script.upload.delivered_bytes(), CHUNK);
    assert_eq!(offsets(&script.send(0, 1)), [CHUNK]);
}

#[test]
fn nothing_beyond_the_window_of_the_first_chunk_missing() {
    let mut script = Script::new(&RecordingSettings {
        in_flight_chunks: 1_000,
        ..settings()
    });
    script.grows_to(1_000 * CHUNK, 0);
    let sent = script.send(0, 50);
    // All but the first delivered: the window (100 chunks) cannot slide past it.
    script.ack(0, sent[49].0, &[(sent[1].0, sent[49].0)]);
    let next = script.send(0, 200);
    assert_eq!(next.first().map(|(_, chunk)| chunk.offset), Some(0));
    assert_eq!(
        next.last().map(|(_, chunk)| chunk.offset),
        Some(99 * CHUNK),
        "chunk 0 again, then up to the window's end"
    );
}

#[test]
fn no_more_in_flight_than_allowed() {
    let mut script = Script::new(&settings());
    script.grows_to(1_000 * CHUNK, 0);
    assert_eq!(script.send(0, 1_000).len(), 50);
    assert_eq!(script.upload.next_chunk(script.now_us), None);
}

#[test]
fn a_tail_outgrown_after_its_delivery_goes_again_whole() {
    let mut script = Script::new(&settings());
    script.grows_to(1_500, 0);
    script.grows_to(1_500, 2_000 * MS);
    let sent = script.send(0, 5);
    assert_eq!(offsets(&sent), [0, 1_000]);
    script.ack(0, sent[1].0, &[(sent[0].0, sent[1].0)]);
    assert!(script.upload.complete());
    script.grows_to(2_200, 100 * MS);
    assert!(!script.upload.complete());
    assert_eq!(
        script.send(0, 1).first().map(|(_, chunk)| *chunk),
        Some(ChunkToSend {
            offset: 1_000,
            len: 1_000
        })
    );
}

#[test]
fn gives_up_after_the_drain_time() {
    let drain = RecordingSettings {
        drain_us: 10_000 * MS,
        ..settings()
    };
    let mut script = Script::new(&drain);
    script.grows_to(3 * CHUNK, 0);
    script.grows_to(3 * CHUNK, 2_000 * MS);
    script.now_us += 10_000 * MS;
    assert!(script.upload.abandoned(script.now_us));
    assert!(script.send(0, 1).is_empty());
}

#[test]
fn sequences_wrap_around() {
    let mut script = Script::starting_at(u32::MAX - 2);
    script.grows_to(10 * CHUNK, 0);
    let sent = script.send(0, 6);
    assert_eq!(sent[3].0, 0, "wrapped");
    script.ack(
        0,
        sent[5].0,
        &[(sent[0].0, sent[2].0), (sent[4].0, sent[5].0)],
    );
    assert_eq!(script.upload.lost_chunks(), 1);
    assert_eq!(script.upload.delivered_bytes(), 5 * CHUNK);
    assert_eq!(offsets(&script.send(0, 1)), [3 * CHUNK]);
}
