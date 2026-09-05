use crate::url_allowed;

#[test]
fn allows_supported_hosts() {
    for url in [
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
        "https://youtu.be/dQw4w9WgXcQ",
        "https://youtube.com/shorts/abc",
        "https://www.instagram.com/reel/xyz/",
        "https://www.tiktok.com/@a/video/1",
        "https://www.facebook.com/reel/1",
        "https://fb.watch/abc",
        "https://x.com/a/status/1",
        "https://twitter.com/a/status/1",
        "https://www.reddit.com/r/x/comments/1",
        "https://v.redd.it/abc",
    ] {
        assert!(url_allowed(url), "{url}");
    }
}

#[test]
fn rejects_other_hosts() {
    assert!(!url_allowed("https://vimeo.com/123"));
    assert!(!url_allowed("https://example.com/a.mp4"));
    assert!(!url_allowed("not-a-url"));
}
