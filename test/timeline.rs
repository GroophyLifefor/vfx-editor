use crate::{
    classify_range_drag, keep_tl_in_view, label_sec_frame, nice_frame_step, nice_sec_step, tl_span,
    tl_view, RangeDrag,
};
use eframe::egui::Rect;

#[test]
fn zoom_narrows_span() {
    let full = tl_span(483, 1.0);
    let zoomed = tl_span(483, 4.0);
    assert!(zoomed < full);
    assert!(zoomed > 0.0);
}

#[test]
fn keep_playhead_in_view_centers() {
    let mut zoom = 8.0;
    let mut scroll = 0.0;
    keep_tl_in_view(&mut zoom, &mut scroll, 400, 483);
    let v = tl_view(483, zoom, scroll);
    assert!(v.start <= 400.0 && 400.0 <= v.start + v.span);
}

#[test]
fn ruler_steps() {
    assert_eq!(nice_sec_step(100.0), 1);
    assert_eq!(nice_frame_step(100.0), 1);
    assert_eq!(label_sec_frame(24, 24.0), "01:00f");
}

#[test]
fn shift_drag_creates_range() {
    let r = Rect::from_min_max([0.0, 0.0].into(), [100.0, 20.0].into());
    let v = tl_view(100, 1.0, 0.0);
    match classify_range_drag(10, Some(10.0), r, None, true, v) {
        Some(RangeDrag::Create { origin }) => assert_eq!(origin, 10),
        _ => panic!("expected Create"),
    }
}
