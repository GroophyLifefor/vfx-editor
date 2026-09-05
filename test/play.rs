use crate::play_resume_frame;

#[test]
fn mid_clip_stays() {
    assert_eq!(play_resume_frame(100, 482, false), 100);
}

#[test]
fn play_from_end_restarts_at_zero() {
    assert_eq!(play_resume_frame(482, 482, false), 0);
    assert_eq!(play_resume_frame(0, 0, false), 0);
}

#[test]
fn eof_before_probed_last_still_restarts() {
    // WhatsApp clip: eof at 1130, probe last=1131
    assert_eq!(play_resume_frame(1130, 1131, true), 0);
    assert_eq!(play_resume_frame(1130, 1131, false), 1130);
}
