use crate::dark_visuals;

#[test]
fn dark_weak_text_is_readable() {
    let d = dark_visuals();
    assert!(d.weak_text_color().r() > 150);
    assert!(d.text_color().r() >= 200);
}
