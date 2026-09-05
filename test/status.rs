use crate::{download_pct, json_quoted, ytdlp_status, Lang};

#[test]
fn download_percent() {
    assert_eq!(
        download_pct("[download]  45.3% of 100.00MiB at 1.2MiB/s ETA 00:12"),
        Some(45)
    );
}

#[test]
fn maps_ytdlp_lines() {
    let s = ytdlp_status("[youtube] abc: Extracting URL", Lang::En).unwrap();
    assert!(s.contains("Extracting"));
    let s = ytdlp_status("[download]  12.0% of 10MiB at 1MiB/s ETA 00:08", Lang::En).unwrap();
    assert!(s.contains("Downloading"));
    assert!(s.contains("12%"));
    let s = ytdlp_status("[Merger] Merging formats into clip.mp4", Lang::En).unwrap();
    assert!(s.contains("Merging"));
}

#[test]
fn json_string_field() {
    assert_eq!(
        json_quoted(r#"{"tag_name":"v0.1.4"}"#, "tag_name").as_deref(),
        Some("v0.1.4")
    );
}
