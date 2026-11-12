//! Writes a synthetic trace with the statistics of a real one, to standard output.
//!
//! Usage: `synthesize_trace <input-trace> <duration-ms> <seed> [bin-ms] [levels]`

use link_emulator::{RateModel, Trace};
use std::error::Error;
use std::fmt::Write as _;
use std::io::Write as _;

const USAGE: &str =
    "usage: synthesize_trace <input-trace> <duration-ms> <seed> [bin-ms=100] [levels=8]";

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, duration, seed, rest @ ..] = args.as_slice() else {
        return Err(USAGE.into());
    };
    let bin_ms: u64 = rest.first().map_or(Ok(100), |value| value.parse())?;
    let levels: usize = rest.get(1).map_or(Ok(8), |value| value.parse())?;
    let original = Trace::parse(&std::fs::read_to_string(input)?)?;
    let synthetic =
        RateModel::fit(&original, bin_ms, levels).generate(duration.parse()?, seed.parse()?)?;
    let mut out = String::with_capacity(synthetic.len() * 6);
    for index in 0..synthetic.len() {
        writeln!(out, "{}", synthetic.opportunity_ms(index))?;
    }
    std::io::stdout().write_all(out.as_bytes())?;
    Ok(())
}
