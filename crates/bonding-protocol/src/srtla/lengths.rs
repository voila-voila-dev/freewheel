//! Fixed lengths of SRTLA packets.

/// Length of the 16-bit type that starts every control packet.
pub const TYPE_LEN: usize = 2;
/// Length of the connection group id exchanged during registration.
pub const ID_LEN: usize = 256;
/// Length of the half of the group id each side chooses (sender first, receiver second).
pub const HALF_ID_LEN: usize = ID_LEN / 2;
/// Smallest valid SRT packet: its fixed header.
pub const SRT_MIN_LEN: usize = 16;
/// Smallest SRT ACK we can read: header plus the last acknowledged sequence number.
pub const SRT_ACK_MIN_LEN: usize = SRT_MIN_LEN + SEQUENCE_NUMBER_LEN;
/// Length of one sequence number on the wire (and of every word of a loss list).
pub const SEQUENCE_NUMBER_LEN: usize = 4;
/// Length of an SRTLA ACK before its sequence numbers: the type and two zero bytes.
pub const SRTLA_ACK_HEADER_LEN: usize = 4;
/// Length of a REG1 packet.
pub const REG1_LEN: usize = TYPE_LEN + ID_LEN;
/// Length of a REG2 packet.
pub const REG2_LEN: usize = TYPE_LEN + ID_LEN;
/// Length of a REG3 packet.
pub const REG3_LEN: usize = TYPE_LEN;
/// Length of a `REG_ERR`, `REG_NGP` or `REG_NAK` packet: the type alone.
pub const REG_ANSWER_LEN: usize = TYPE_LEN;
/// Largest datagram either side reads (upstream's receive buffer, one Ethernet MTU).
pub const MAX_DATAGRAM_LEN: usize = 1_500;
/// Most sequence numbers one SRTLA ACK can carry within [`MAX_DATAGRAM_LEN`].
pub const MAX_SRTLA_ACK_SEQUENCE_NUMBERS: usize =
    (MAX_DATAGRAM_LEN - SRTLA_ACK_HEADER_LEN) / SEQUENCE_NUMBER_LEN;
