use crate::parse_ver;

#[test]
fn strips_v_and_pads() {
    assert_eq!(parse_ver("v0.1.4"), (0, 1, 4));
    assert_eq!(parse_ver("0.1"), (0, 1, 0));
    assert!(parse_ver("0.1.4") > parse_ver("0.1.3"));
}
