use super::*;
#[test]
fn english_labels_are_exact_static_and_keys_have_one_sorted_identity() {
    let label: &'static str = text("action.save");
    assert_eq!(label, "Save");
    assert_eq!(text("action.save_shortcut"), "Save   Ctrl+S");
    assert_eq!(text("page.character_knowledge"), "Character knowledge");
    assert_eq!(text("page.reader_timeline"), "Reader learns timeline");
    assert!(ENGLISH.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert!(ENGLISH.iter().all(|(key, value)| !key.is_empty() && !value.is_empty()));
}
#[test]
fn unknown_keys_and_unavailable_locales_have_explicit_offline_fallbacks() {
    assert_eq!(get("Action.Save"), None);
    assert_eq!(text("missing.interface.key"), "Interface text unavailable");
    assert_eq!(text_for("ar", "action.cancel"), "Cancel");
    assert_eq!(locale_notice("en"), None);
    assert_eq!(locale_notice("ar"), Some("This interface language is unavailable. English labels are shown."));
}
#[test]
fn lookup_does_not_translate_normalize_or_mutate_author_content_or_route_ids() {
    let authored = "نور 明 e\u{301} \u{e000} 👩🏽‍🚀".to_owned();
    let original = authored.clone();
    assert_eq!(get(&authored), None);
    assert_eq!(authored, original);
    // Display captions have their own keys; serialized IDs remain external.
    assert_eq!(get("planning"), None);
    assert_eq!(get("zh-Hans"), None);
    assert_eq!(text("area.planning"), "Planning");
}
