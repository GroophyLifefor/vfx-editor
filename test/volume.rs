use crate::parse_volume;

#[test]
fn percent_file() {
    assert_eq!(parse_volume("80"), Some(0.8));
    assert_eq!(parse_volume("100"), Some(1.0));
    assert_eq!(parse_volume("125"), Some(1.25));
    assert_eq!(parse_volume("0"), Some(0.0));
    assert_eq!(parse_volume(" 90 \n"), Some(0.9));
}

#[test]
fn rejects_junk() {
    assert!(parse_volume("").is_none());
    assert!(parse_volume("x").is_none());
    assert!(parse_volume("200").is_none());
    assert!(parse_volume("-1").is_none());
}
