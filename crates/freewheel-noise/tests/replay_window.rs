//! The replay window: each nonce once, in any order within the window.

use freewheel_noise::ReplayWindow;

#[test]
fn sizes_round_up_to_whole_words() {
    assert_eq!(ReplayWindow::new(0).packets(), 64);
    assert_eq!(ReplayWindow::new(64).packets(), 64);
    assert_eq!(ReplayWindow::new(65).packets(), 128);
}

#[test]
fn each_nonce_once_in_any_order() {
    let mut window = ReplayWindow::new(128);
    for nonce in [5, 3, 9, 4, 0, 127] {
        assert!(window.is_fresh(nonce), "{nonce}");
        window.mark(nonce);
        assert!(!window.is_fresh(nonce), "{nonce}");
    }
    assert!(window.is_fresh(1) && window.is_fresh(126));
}

#[test]
fn the_window_slides_with_the_highest_nonce() {
    let mut window = ReplayWindow::new(64);
    window.mark(10);
    window.mark(70);
    assert!(!window.is_fresh(6), "below the window");
    assert!(window.is_fresh(7) && window.is_fresh(69));
    assert!(!window.is_fresh(10), "seen, and still within the window");
    window.mark(73);
    assert!(!window.is_fresh(10), "now below the window");
    assert!(window.is_fresh(71) && window.is_fresh(72));
}

#[test]
fn a_jump_beyond_the_window_forgets_everything_below() {
    let mut window = ReplayWindow::new(64);
    for nonce in 0..64 {
        window.mark(nonce);
    }
    window.mark(1_000);
    assert!(!window.is_fresh(936));
    for nonce in 937..1_000 {
        assert!(window.is_fresh(nonce), "{nonce}");
    }
    assert!(!window.is_fresh(1_000));
}

#[test]
fn bits_reused_by_the_ring_are_cleared() {
    let mut window = ReplayWindow::new(64);
    window.mark(0);
    window.mark(1);
    window.mark(64);
    // 64 takes 0's bit; 65 would take 1's, and must be fresh.
    assert!(window.is_fresh(65));
    window.mark(65);
    assert!(!window.is_fresh(65) && !window.is_fresh(1));
}

#[test]
fn reset_forgets_every_nonce() {
    let mut window = ReplayWindow::new(64);
    window.mark(500);
    window.reset();
    assert!(window.is_fresh(0) && window.is_fresh(500));
}
