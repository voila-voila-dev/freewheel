//! Keeping bitrates inside their bounds.

use bitrate_control::Bounds;

#[test]
fn clamps_into_bounds() {
    let bounds = Bounds {
        min_bps: 500_000,
        max_bps: 6_000_000,
    };
    assert_eq!(bounds.clamp(100), 500_000);
    assert_eq!(bounds.clamp(3_000_000), 3_000_000);
    assert_eq!(bounds.clamp(9_000_000), 6_000_000);
}
