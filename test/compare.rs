use crate::compare::{
    estimate_bytes, fps_differs, map_compare_frame, parse_devices, parse_progress, timeline_len,
};

#[test]
fn same_fps_maps_one_to_one() {
    assert_eq!(map_compare_frame(10, 24.0, 100, 24.0), 10);
    assert_eq!(map_compare_frame(200, 24.0, 50, 24.0), 49);
}

#[test]
fn time_maps_when_fps_differs() {
    assert_eq!(map_compare_frame(24, 24.0, 200, 48.0), 48);
}

#[test]
fn longer_clip_sets_timeline() {
    assert_eq!(timeline_len(24, 24.0, 48, 24.0), 48);
    assert_eq!(timeline_len(48, 24.0, 24, 24.0), 48);
}

#[test]
fn parse_gpu_lines() {
    let t = "[0 Intel(R) UHD Graphics]  queueC=0\n[1 NVIDIA GeForce GTX 1650]  queueC=2\n";
    let d = parse_devices(t);
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].0, 0);
    assert!(d[1].1.contains("1650"));
    let v64 = "0. Intel(R) UHD Graphics\n\tType: Integrated GPU\n1. NVIDIA GeForce GTX 1650\n";
    let d = parse_devices(v64);
    assert_eq!(d.len(), 2);
    assert_eq!(d[1].0, 1);
    assert!(d[1].1.contains("1650"));
}

#[test]
fn parse_frame_progress() {
    assert_eq!(
        parse_progress("frame=127/129 (98.45%); fps=0.65"),
        Some((127, 129))
    );
}

#[test]
fn estimate_grows_with_scale() {
    assert!(estimate_bytes(1_000, 4, 1.0) > estimate_bytes(1_000, 2, 1.0));
    assert!(!fps_differs(24.0, 24.0));
    assert!(fps_differs(24.0, 30.0));
}
