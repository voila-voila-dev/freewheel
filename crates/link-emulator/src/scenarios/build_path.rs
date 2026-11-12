//! Builds one SIM path from a capacity model, the way every scenario needs it.

use crate::{Event, GilbertElliott, LinkConfig, Path, RadioPower, RateModel, Trace, TraceError};

/// One SIM: synthetic uplink from `uplink`, steady downlink for acknowledgements.
pub(crate) fn build_path(
    uplink: &RateModel,
    duration_ms: u64,
    propagation_ms: u32,
    loss: GilbertElliott,
    events: &[Event],
    seed: u64,
) -> Result<Path, TraceError> {
    let up = LinkConfig {
        trace: uplink.generate(duration_ms, seed)?,
        propagation_ms,
        queue_bytes: 300_000,
        loss,
        events: Vec::new(),
    };
    let down = LinkConfig {
        trace: Trace::constant(2, duration_ms.min(10_000))?,
        propagation_ms,
        queue_bytes: 300_000,
        loss: GilbertElliott::bernoulli(0.005),
        events: Vec::new(),
    };
    Ok(Path::new(up, down, events, RadioPower::lte_typical(), seed))
}
