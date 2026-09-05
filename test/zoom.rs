use crate::{remap_pan, view_scale, ZoomMode, ZOOM_MAX};
use eframe::egui::Vec2;

#[test]
fn fit_and_clamp() {
    assert!((view_scale(200.0, 100.0, 100.0, 100.0, ZoomMode::FitWidth, 1.0) - 2.0).abs() < 1e-5);
    assert!((view_scale(200.0, 100.0, 100.0, 100.0, ZoomMode::FitHeight, 1.0) - 1.0).abs() < 1e-5);
    assert!((view_scale(200.0, 100.0, 100.0, 100.0, ZoomMode::Contain, 1.0) - 1.0).abs() < 1e-5);
    assert_eq!(
        view_scale(8000.0, 8000.0, 100.0, 100.0, ZoomMode::FitWidth, 1.0),
        ZOOM_MAX
    );
    assert!((view_scale(200.0, 100.0, 100.0, 100.0, ZoomMode::Manual, 4.0) - 4.0).abs() < 1e-5);
}

#[test]
fn zoom_past_fit_stays_centered() {
    let avail = Vec2::splat(200.0);
    let old = Vec2::splat(100.0);
    let new = Vec2::splat(400.0);
    let pan = remap_pan(Vec2::ZERO, old, new, avail, avail * 0.5);
    assert!((pan - Vec2::splat(100.0)).length() < 1e-3);
}
