use crate::tour::{duration, BEATS};

#[test]
fn beats_sum_to_thirty() {
    let s = duration();
    assert!((s - 30.0).abs() < 0.05, "tour is {s}s, want 30");
}

#[test]
fn beat_ids_unique() {
    let mut ids: Vec<_> = BEATS.iter().map(|b| b.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), BEATS.len());
}
