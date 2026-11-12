//! A fixed, realistic mix of packets, built once before measuring.

use bonding_protocol::srtla::{PacketType, REG1_LEN};

/// Mix of data packets, keepalives, ACKs and registrations, as seen on a live link.
#[derive(Debug, Clone)]
pub struct SamplePackets {
    packets: Vec<Vec<u8>>,
}

impl SamplePackets {
    /// Builds the mix: mostly 1316-byte SRT data packets, some control packets.
    #[must_use]
    pub fn new() -> Self {
        let mut packets = Vec::new();
        for sequence in 0u32..60 {
            let mut data = vec![0u8; 1316];
            data[..4].copy_from_slice(&sequence.to_be_bytes());
            packets.push(data);
        }
        for kind in [
            PacketType::Keepalive,
            PacketType::SrtlaAck,
            PacketType::SrtAck,
        ] {
            let mut control = vec![0u8; 16];
            control[..2].copy_from_slice(&(kind as u16).to_be_bytes());
            packets.push(control);
        }
        let mut reg1 = vec![0u8; REG1_LEN];
        reg1[..2].copy_from_slice(&(PacketType::Reg1 as u16).to_be_bytes());
        packets.push(reg1);
        Self { packets }
    }

    /// The `index`-th packet, cycling through the mix.
    #[must_use]
    pub fn get(&self, index: u64) -> &[u8] {
        let len = self.packets.len() as u64;
        let slot = usize::try_from(index % len).unwrap_or(0);
        self.packets.get(slot).map_or(&[], Vec::as_slice)
    }
}

impl Default for SamplePackets {
    fn default() -> Self {
        Self::new()
    }
}
