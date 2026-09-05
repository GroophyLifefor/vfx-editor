use crate::{crash_path, format_crash, lang_path};

#[test]
fn crash_sits_next_to_lang() {
    assert_eq!(crash_path().parent(), lang_path().parent());
    assert_eq!(crash_path().file_name().unwrap(), "crash.log");
}

#[test]
fn crash_text_has_panic_then_dump() {
    let s = format_crash("panic: boom", "VFX Player v0.1.5\n[    0.00] start");
    assert!(s.starts_with("panic: boom"));
    assert!(s.contains("---"));
    assert!(s.contains("VFX Player"));
}
