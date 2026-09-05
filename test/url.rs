use crate::url_ext;

#[test]
fn picks_known_or_mp4() {
    assert_eq!(url_ext("https://x.com/a.mkv?token=1"), "mkv");
    assert_eq!(url_ext("https://x.com/a.MOV"), "mov");
    assert_eq!(url_ext("https://x.com/noext"), "mp4");
}
