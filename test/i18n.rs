use crate::Lang;

#[test]
fn tr_en_and_parse() {
    assert_eq!(Lang::Tr.tr("Aç", "Open"), "Aç");
    assert_eq!(Lang::En.tr("Aç", "Open"), "Open");
    assert!(matches!(Lang::parse("tr"), Some(Lang::Tr)));
    assert!(matches!(Lang::parse("en"), Some(Lang::En)));
    assert!(Lang::parse("de").is_none());
}
