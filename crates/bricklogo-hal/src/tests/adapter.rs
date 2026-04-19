use super::*;

// ── rotate_abs_delta ──────────────────────────────
//
// Delta is a function of APOS, target, and direction. The motor rotates
// less than one full revolution toward the target absolute angle in the
// specified direction.

#[test]
fn test_rotate_abs_already_at_target() {
    assert_eq!(rotate_abs_delta(0, 0, PortDirection::Even), 0);
    assert_eq!(rotate_abs_delta(0, 0, PortDirection::Odd), 0);
    assert_eq!(rotate_abs_delta(90, 90, PortDirection::Even), 0);
    assert_eq!(rotate_abs_delta(90, 90, PortDirection::Odd), 0);
}

#[test]
fn test_rotate_abs_to_zero_positive_apos() {
    // Equivalent to old rotate_home_delta(60, dir) with target=0
    assert_eq!(rotate_abs_delta(60, 0, PortDirection::Odd), -60);
    assert_eq!(rotate_abs_delta(60, 0, PortDirection::Even), 300);
}

#[test]
fn test_rotate_abs_to_zero_negative_apos() {
    assert_eq!(rotate_abs_delta(-60, 0, PortDirection::Even), 60);
    assert_eq!(rotate_abs_delta(-60, 0, PortDirection::Odd), -300);
}

#[test]
fn test_rotate_abs_to_zero_boundary() {
    assert_eq!(rotate_abs_delta(180, 0, PortDirection::Even), 180);
    assert_eq!(rotate_abs_delta(180, 0, PortDirection::Odd), -180);
    assert_eq!(rotate_abs_delta(-180, 0, PortDirection::Even), 180);
    assert_eq!(rotate_abs_delta(-180, 0, PortDirection::Odd), -180);
}

#[test]
fn test_rotate_abs_to_nonzero_target() {
    // From 0 to 90: forward is +90, backward is -270
    assert_eq!(rotate_abs_delta(0, 90, PortDirection::Even), 90);
    assert_eq!(rotate_abs_delta(0, 90, PortDirection::Odd), -270);
}

#[test]
fn test_rotate_abs_backward_to_target() {
    // From 90 to 0: backward is -90, forward is +270
    assert_eq!(rotate_abs_delta(90, 0, PortDirection::Odd), -90);
    assert_eq!(rotate_abs_delta(90, 0, PortDirection::Even), 270);
}

#[test]
fn test_rotate_abs_across_boundary() {
    // From -170 to 170: forward crosses 360 boundary (+340), backward is -20
    assert_eq!(rotate_abs_delta(-170, 170, PortDirection::Even), 340);
    assert_eq!(rotate_abs_delta(-170, 170, PortDirection::Odd), -20);
}

#[test]
fn test_rotate_abs_bounded_to_one_revolution() {
    // Out-of-range APOS values still produce sub-360 deltas
    assert_eq!(rotate_abs_delta(700, 0, PortDirection::Even), 20);
    assert_eq!(rotate_abs_delta(700, 0, PortDirection::Odd), -340);
    assert_eq!(rotate_abs_delta(-700, 0, PortDirection::Even), 340);
    assert_eq!(rotate_abs_delta(-700, 0, PortDirection::Odd), -20);
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
