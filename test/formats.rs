use crate::{build_format_opts, codec_a, codec_v};

#[test]
fn codec_names() {
    assert_eq!(codec_v("avc1.640028"), Some("H264"));
    assert_eq!(codec_v("vp9"), Some("VP9"));
    assert_eq!(codec_v("av01.0"), Some("AV1"));
    assert_eq!(codec_v("none"), None);
    assert_eq!(codec_a("mp4a.40.2"), Some("AAC"));
    assert_eq!(codec_a("opus"), Some("Opus"));
    assert_eq!(codec_a("none"), None);
}

#[test]
fn pairs_video_with_audio() {
    let json = r#"[
      {"format_id":"140","vcodec":"none","acodec":"mp4a.40.2","tbr":130},
      {"format_id":"251","vcodec":"none","acodec":"opus","tbr":160},
      {"format_id":"137","height":1080,"vcodec":"avc1.640028","acodec":"none","tbr":4500},
      {"format_id":"248","height":1080,"vcodec":"vp9","acodec":"none","tbr":3000}
    ]"#;
    let opts = build_format_opts(json);
    let labels: Vec<_> = opts.iter().map(|o| o.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.contains("1080p H264 + AAC")), "{labels:?}");
    assert!(labels.iter().any(|l| l.contains("1080p VP9 + Opus")), "{labels:?}");
    assert!(opts.iter().any(|o| o.spec.contains('+')));
}
