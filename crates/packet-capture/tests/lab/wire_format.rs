//! Our decoder on every datagram upstream exchanged, in every scenario.
//!
//! Upstream never sends anything we cannot read; only the scenarios that inject hostile
//! packets contain datagrams we refuse, and those are refused, never misread.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bonding_protocol::srtla::{MAX_DATAGRAM_LEN, Packet};
use packet_capture::{LAB_RECEIVER_PORT, PcapFile, UdpDatagram};

const CAPTURES_DIR: &str = "../../.cache/srtla-captures";
const HOSTILE_SCENARIOS: &[&str] = &["hostile-packets", "group-limit", "group-hijack"];

#[test]
fn every_lab_datagram_decodes_and_encodes_back() {
    let directory = std::env::var_os("FREEWHEEL_CAPTURES_DIR")
        .map_or_else(|| PathBuf::from(CAPTURES_DIR), PathBuf::from);
    let Ok(entries) = std::fs::read_dir(&directory) else {
        assert!(
            std::env::var_os("FREEWHEEL_REQUIRE_CAPTURES").is_none(),
            "no captures in {directory:?} (run labs/srtla-capture/run-all.sh)"
        );
        eprintln!("skipping: run labs/srtla-capture/run-all.sh first");
        return;
    };
    let mut captures: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".pcap"))
        .collect();
    captures.sort();
    assert!(!captures.is_empty(), "no .pcap in {directory:?}");

    let mut out = [0u8; MAX_DATAGRAM_LEN];
    for capture in captures {
        let name = capture
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Ok(bytes) = std::fs::read(&capture) else {
            unreachable!("{name}: unreadable");
        };
        let Ok(file) = PcapFile::parse(&bytes) else {
            unreachable!("{name}: not a pcap file");
        };
        let Ok(frames) = file.frames() else {
            unreachable!("{name}: truncated");
        };
        let mut refused: BTreeMap<String, u64> = BTreeMap::new();
        let mut decoded = 0u64;
        for datagram in frames.into_iter().filter_map(UdpDatagram::decode) {
            if datagram.source.port() != LAB_RECEIVER_PORT
                && datagram.destination.port() != LAB_RECEIVER_PORT
            {
                continue;
            }
            match Packet::decode(datagram.payload) {
                Ok(packet) => {
                    let Ok(len) = packet.encode(&mut out) else {
                        unreachable!("{name}: {packet:?} does not encode");
                    };
                    assert_eq!(&out[..len], datagram.payload, "{name}: {packet:?}");
                    decoded += 1;
                }
                Err(error) => *refused.entry(format!("{error:?}")).or_default() += 1,
            }
        }
        assert!(decoded > 0, "{name}: no SRTLA traffic");
        let hostile = HOSTILE_SCENARIOS
            .iter()
            .any(|scenario| name.starts_with(scenario));
        assert!(
            refused.is_empty() || hostile,
            "{name}: refused {refused:?} although upstream sent it"
        );
    }
}
