use super::*;

// ── rotate_home_delta ──────────────────────────────
//
// Delta is a function of APOS and direction only — POS does not enter the
// computation. The motor rotates less than one full revolution toward
// mechanical home (APOS=0) in the specified direction.

#[test]
fn test_rotate_home_already_at_home() {
    assert_eq!(rotate_home_delta(0, PortDirection::Even), 0);
    assert_eq!(rotate_home_delta(0, PortDirection::Odd), 0);
}

#[test]
fn test_rotate_home_positive_apos() {
    // APOS=60 (60° forward of home). Shortest backward path: -60°.
    // Forward path must go the long way around: +300°.
    assert_eq!(rotate_home_delta(60, PortDirection::Odd), -60);
    assert_eq!(rotate_home_delta(60, PortDirection::Even), 300);
}

#[test]
fn test_rotate_home_negative_apos() {
    // APOS=-60 (60° backward of home). Short forward path: +60°.
    // Backward path must go the long way: -300°.
    assert_eq!(rotate_home_delta(-60, PortDirection::Even), 60);
    assert_eq!(rotate_home_delta(-60, PortDirection::Odd), -300);
}

#[test]
fn test_rotate_home_apos_boundary() {
    assert_eq!(rotate_home_delta(180, PortDirection::Even), 180);
    assert_eq!(rotate_home_delta(180, PortDirection::Odd), -180);
    assert_eq!(rotate_home_delta(-180, PortDirection::Even), 180);
    assert_eq!(rotate_home_delta(-180, PortDirection::Odd), -180);
}

#[test]
fn test_rotate_home_bounded_to_one_revolution() {
    // If APOS comes in outside the nominal [-180, 180] range (bad read,
    // stale data, whatever), the delta must still be bounded to a single
    // revolution — rem_euclid(360) guarantees that.
    assert_eq!(rotate_home_delta(700, PortDirection::Even), 20);
    assert_eq!(rotate_home_delta(700, PortDirection::Odd), -340);
    assert_eq!(rotate_home_delta(-700, PortDirection::Even), 340);
    assert_eq!(rotate_home_delta(-700, PortDirection::Odd), -20);
}

// ── rotateto_delta ─────────────────────────────────

#[test]
fn test_rotateto_delta_basic_forward() {
    assert_eq!(rotateto_delta(0, 90, PortDirection::Even), 90);
    assert_eq!(rotateto_delta(90, 0, PortDirection::Even), 270);
}

#[test]
fn test_rotateto_delta_basic_backward() {
    assert_eq!(rotateto_delta(0, 90, PortDirection::Odd), -270);
    assert_eq!(rotateto_delta(90, 0, PortDirection::Odd), -90);
}

#[test]
fn test_rotateto_delta_already_at_target() {
    assert_eq!(rotateto_delta(90, 90, PortDirection::Even), 0);
    assert_eq!(rotateto_delta(90, 90, PortDirection::Odd), 0);
    assert_eq!(rotateto_delta(450, 90, PortDirection::Even), 0);
}

#[test]
fn test_rotateto_delta_wraps_mod_360() {
    // Cumulative encoder 720 = same angle as 0.
    assert_eq!(rotateto_delta(720, 90, PortDirection::Even), 90);
}
