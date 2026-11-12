//! Same inputs and seed, same result.

mod support;

use link_emulator::GilbertElliott;
use support::{constant, link, run};

#[test]
fn same_seed_same_result() {
    let make = || {
        link(
            constant(1),
            200_000,
            GilbertElliott::new(0.01, 0.2, 0.0, 0.5),
            vec![],
            42,
        )
    };
    let (a, _) = run(&mut make(), 1, 3_000, None);
    let (b, _) = run(&mut make(), 1, 3_000, None);
    assert_eq!(a, b);
}
