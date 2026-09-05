use crate::play_resume_frame;

#[test]
fn mid_clip_stays() {
    assert_eq!(play_resume_frame(100, 482), 100);
}

/// issue #1 — Play at the last frame should restart at 0.
/// Fails until the fix is approved.
#[test]
fn play_from_end_restarts_at_zero() {
    assert_eq!(play_resume_frame(482, 482), 0);
    assert_eq!(play_resume_frame(0, 0), 0);
}
