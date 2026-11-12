//! The file of source addresses, one per modem, that belaUI writes for the sender.

use std::fs;
use std::io;
use std::net::Ipv4Addr;
use std::path::Path;

/// Upstream reads addresses with `inet_addr`, which cannot tell this one from an error.
const REFUSED_ADDRESS: Ipv4Addr = Ipv4Addr::BROADCAST;

/// The addresses in a source list, in file order, as upstream reads them: one dotted quad at the
/// start of each line (anything after a blank is ignored); other lines are skipped.
#[must_use]
pub fn parse_source_list(text: &[u8]) -> Vec<Ipv4Addr> {
    text.split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let first_word = line.split(u8::is_ascii_whitespace).next()?;
            std::str::from_utf8(first_word)
                .ok()?
                .parse::<Ipv4Addr>()
                .ok()
        })
        .filter(|address| *address != REFUSED_ADDRESS)
        .collect()
}

/// Reads and parses a source list.
///
/// # Errors
/// The file cannot be read.
pub fn read_source_list(path: &Path) -> io::Result<Vec<Ipv4Addr>> {
    Ok(parse_source_list(&fs::read(path)?))
}
