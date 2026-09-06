use std::ffi::OsString;

use crate::collect_open_paths;

#[test]
fn one_path_is_open_only() {
    let (a, b) = collect_open_paths([OsString::from(r"C:\x\shot.mp4")].into_iter());
    assert!(a.unwrap().ends_with("shot.mp4"));
    assert!(b.is_none());
}

#[test]
fn two_videos_fill_compare() {
    let (a, b) = collect_open_paths(
        [
            OsString::from(r"C:\x\a.mp4"),
            OsString::from(r"C:\x\b.mov"),
        ]
        .into_iter(),
    );
    assert!(a.unwrap().ends_with("a.mp4"));
    assert!(b.unwrap().ends_with("b.mov"));
}

#[test]
fn intro_video_dest_is_not_open() {
    let (a, b) = collect_open_paths(
        [
            OsString::from("--intro-video"),
            OsString::from(r"C:\x\demo.mp4"),
            OsString::from(r"C:\x\clip.mp4"),
        ]
        .into_iter(),
    );
    assert!(a.unwrap().ends_with("clip.mp4"));
    assert!(b.is_none());
}
