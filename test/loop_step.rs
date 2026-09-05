use crate::loop_step;

#[test]
fn plus_ten_hits_out_then_wraps_from_in() {
    let mut f = 15u64;
    let seq: Vec<u64> = (0..4)
        .map(|_| {
            f = loop_step(f, 10, 15, 33);
            f
        })
        .collect();
    assert_eq!(seq, [25, 33, 25, 33]);
}

#[test]
fn plus_ten_tiny_range_clamps_at_out() {
    let mut f = 15u64;
    let seq: Vec<u64> = (0..4)
        .map(|_| {
            f = loop_step(f, 10, 15, 23);
            f
        })
        .collect();
    assert_eq!(seq, [23, 23, 23, 23]);
}

#[test]
fn minus_one_from_in_lands_before_out() {
    // overshoot: I + -1 → O-1, not modulo wrap to O
    assert_eq!(loop_step(15, -1, 15, 33), 32);
}

#[test]
fn outside_range_does_not_wrap() {
    assert_eq!(loop_step(5, 10, 15, 33), 15);
}
