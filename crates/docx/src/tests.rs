use super::*;
use serde_json::json;
use std::io::{Cursor, Read};

fn fixture() -> ProjectBundle {
    serde_json::from_str(include_str!("../../../tests/fixtures/docx-project.json")).unwrap()
}
fn xml(bytes: &[u8]) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive.by_name("word/document.xml").unwrap().read_to_string(&mut text).unwrap();
    text
}
fn part(bytes: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut text = String::new();
    archive.by_name(name).unwrap().read_to_string(&mut text).unwrap();
    text
}
fn paragraph_texts(document: &Document) -> Vec<String> {
    document.body.iter().map(|block| block.as_para().unwrap().plain_text()).collect()
}

#[test]
fn saved_snapshot_order_chapter_zero_and_source_remain_exact() {
    let bundle = fixture();
    let original = serde_json::to_vec(&bundle).unwrap();
    let doc = manuscript_document(&bundle).unwrap();
    let texts = paragraph_texts(&doc);
    assert_eq!(texts[0], bundle.project.title);
    assert_eq!(texts[1], "Chapter 0 — First / الفصل الأول");
    assert_eq!(texts[2], "Same position, earlier ID");
    assert_eq!(texts[4], "A multilingual scene — قصة 中文");
    assert!(texts.iter().any(|text| text == "Chapter 1 — Second / الفصل الثاني"));
    let bytes = export_manuscript(&bundle).unwrap();
    let reread = wordcraft_docx::read(&bytes).unwrap();
    assert_eq!(paragraph_texts(&reread), texts);
    assert_eq!(serde_json::to_vec(&bundle).unwrap(), original);
    assert_eq!(doc.body.iter().filter_map(|b| b.as_para()).filter(|p| p.props.page_break_before == Some(true)).count(), 1);
}

#[test]
fn actual_docx_contains_multilingual_text_spacing_language_and_complex_script_marks() {
    let output = xml(&export_manuscript(&fixture()).unwrap());
    for text in ["سلامٌ مِمِمِ", "简体中文故事", "繁體中文故事", "👩🏽‍🚀", "e\u{301}", "\u{e000}"] { assert!(output.contains(text)); }
    for value in ["240", "276", "360", "480"] { assert!(output.contains(&format!("w:line=\"{value}\""))); }
    for lang in ["en", "ar", "zh-Hans", "zh-Hant", "und"] { assert!(output.contains(&format!("w:val=\"{lang}\""))); }
    for mark in ["<w:b/>", "<w:bCs/>", "<w:i/>", "<w:iCs/>", "w:u w:val=\"single\""] { assert!(output.contains(mark)); }
    assert!(output.contains("<w:br/>") && output.contains("w:before=\"0\" w:after=\"0\""));
}

#[test]
fn auto_direction_uses_first_strong_character_and_explicit_ltr_wins() {
    for (text, direction, expected) in [("123 — العربية", "auto", true), ("English العربية", "auto", false), ("אבג 123", "auto", true), ("العربية", "ltr", false), ("English", "rtl", true)] {
        let block: SourceBlock = serde_json::from_value(json!({"type":"paragraph","attrs":{"dir":direction,"lang":"und"},"content":[{"type":"text","text":text}]})).unwrap();
        assert_eq!(source_paragraph(block, &ExportProfile::default()).unwrap().props.bidi, Some(expected));
    }
}

#[test]
fn reserved_object_character_tab_carriage_return_and_markup_are_preserved() {
    let mut bundle = fixture();
    let exact = "Literal \u{fffc} \t \r <tag>&\" apostrophe' \u{00ad} \u{2011}";
    bundle.scenes[0].scene.document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":exact,"marks":[{"type":"underline"}]}]}]});
    bundle.project.title.push('\u{fffc}');
    let document = manuscript_document(&bundle).unwrap();
    assert!(paragraph_texts(&document).iter().any(|text| text == exact));
    let output = xml(&export_manuscript(&bundle).unwrap());
    assert!(output.contains('\u{fffc}') && output.contains("<w:tab/>") && output.contains("&#13;"));
    assert!(output.contains("&lt;tag&gt;&amp;&quot;") && !output.contains("<tag>"));
}

#[test]
fn invalid_source_is_refused_instead_of_flattened_or_silently_dropped() {
    for document in [
        json!({"type":"doc","content":[{"type":"table","content":[]}]}),
        json!({"type":"doc","content":[{"type":"paragraph","attrs":{"custom":"unsupported"}}]}),
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"x","marks":[{"type":"bold"},{"type":"bold"}]}]}]}),
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"x\u{fffe}"}]}]}),
        json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"x\u{000b}"}]}]}),
        json!({"type":"doc","content":[{"type":"heading","attrs":{"level":0}}]}),
    ] {
        let mut bundle = fixture();
        bundle.scenes[0].scene.document = document;
        let before = serde_json::to_vec(&bundle).unwrap();
        assert!(export_manuscript(&bundle).is_err());
        assert_eq!(serde_json::to_vec(&bundle).unwrap(), before);
    }
}

#[test]
fn invalid_identity_placement_and_conflicted_numbering_are_refused() {
    let bundle = fixture();
    let mut invalid = bundle.clone(); invalid.scenes[0].chapter_id = "missing".into(); assert!(export_manuscript(&invalid).is_err());
    let mut invalid = bundle.clone(); invalid.scenes[0].id = invalid.scenes[1].id.clone(); assert!(export_manuscript(&invalid).is_err());
    let mut invalid = bundle.clone(); invalid.chapters[0].project_id = "other-project".into(); assert!(export_manuscript(&invalid).is_err());
    let mut invalid = bundle.clone(); invalid.chapter_numbering_conflicted = true; assert!(export_manuscript(&invalid).unwrap_err().contains("Recovery"));
    let mut invalid = bundle; invalid.scenes[0].scene.schema_version = 2; assert!(export_manuscript(&invalid).is_err());
}

#[test]
fn empty_project_and_empty_paragraph_are_valid_exports() {
    let mut bundle = fixture();
    bundle.scenes.clear(); bundle.chapters.clear();
    let bytes = export_manuscript(&bundle).unwrap();
    assert_eq!(paragraph_texts(&wordcraft_docx::read(&bytes).unwrap()), vec![bundle.project.title]);
    let paragraph = source_paragraph(serde_json::from_value(json!({"type":"paragraph","attrs":{"lang":"ar","dir":"rtl"}})).unwrap(), &ExportProfile::default()).unwrap();
    assert!(paragraph.text.is_empty());
    assert_eq!(paragraph.mark.lang.as_deref(), Some("ar"));
}

#[test]
fn full_length_novel_exports_without_truncating_source() {
    let mut bundle = fixture();
    bundle.scenes.truncate(1);
    let novel = "novel ".repeat(250_000);
    bundle.scenes[0].scene.document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":novel}]}]});
    let bytes = export_manuscript(&bundle).unwrap();
    let document = wordcraft_docx::read(&bytes).unwrap();
    assert!(paragraph_texts(&document).iter().any(|text| text == &novel));
}

#[test]
fn literal_object_expansion_is_bounded_before_allocation() {
    let mut bundle = fixture();
    bundle.scenes.truncate(1);
    bundle.scenes[0].scene.document = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"\u{fffc}".repeat(OBJECT_LIMIT + 1)}]}]});
    let original = serde_json::to_vec(&bundle).unwrap();
    assert!(export_manuscript(&bundle).unwrap_err().contains("literal-object limit"));
    assert_eq!(serde_json::to_vec(&bundle).unwrap(), original);
}

#[test]
fn titles_and_source_share_one_literal_object_budget() {
    let mut bundle = fixture();
    bundle.scenes.clear();
    bundle.chapters.truncate(1);
    bundle.project.title = "\u{fffc}".repeat(OBJECT_LIMIT / 2 + 1);
    bundle.chapters[0].title = bundle.project.title.clone();
    assert!(export_manuscript(&bundle).unwrap_err().contains("literal-object limit"));
}

#[test]
fn default_profile_keeps_the_legacy_document_and_export_api() {
    let bundle = fixture();
    let profile = ExportProfile::default();
    assert_eq!(manuscript_document(&bundle).unwrap(), manuscript_document_with_profile(&bundle, &profile).unwrap());
    assert_eq!(export_manuscript(&bundle).unwrap(), export_manuscript_with_profile(&bundle, &profile).unwrap());
}

#[test]
fn profile_controls_actual_ooxml_styles_page_layout_and_body_spacing_only() {
    let bundle = fixture();
    let mut profile = ExportProfile::default();
    profile.page_size = "a4".into();
    profile.margin_mm = 20.0;
    profile.line_spacing = "1.5".into();
    profile.title = ExportStyle { font: "Novel Title Face".into(), size: 30.0, bold: true, italic: true };
    profile.chapter = ExportStyle { font: "Novel Chapter Face".into(), size: 22.0, bold: true, italic: false };
    profile.scene = ExportStyle { font: "Novel Scene Face".into(), size: 18.0, bold: false, italic: true };
    profile.body = ExportStyle { font: "Novel Body Face".into(), size: 11.0, bold: false, italic: false };
    let output = export_manuscript_with_profile(&bundle, &profile).unwrap();
    let styles = part(&output, "word/styles.xml");
    for (id, font, size) in [
        ("Title", "Novel Title Face", "60"),
        ("Heading1", "Novel Chapter Face", "44"),
        ("Heading2", "Novel Scene Face", "36"),
        ("Normal", "Novel Body Face", "22"),
    ] {
        let marker = format!("w:styleId=\"{id}\"");
        let start = styles[..styles.find(&marker).unwrap()].rfind("<w:style").unwrap();
        let end = styles[styles.find(&marker).unwrap()..].find("</w:style>").unwrap() + styles.find(&marker).unwrap();
        let element = &styles[start..end];
        assert!(element.contains(&format!("w:ascii=\"{font}\"")), "{id}: {element}");
        assert!(element.contains(&format!("w:sz w:val=\"{size}\"")), "{id}: {element}");
        assert!(element.contains(&format!("w:szCs w:val=\"{size}\"")), "{id}: {element}");
        let (bold, italic) = match id {
            "Title" => (true, true),
            "Heading1" => (true, false),
            "Heading2" => (false, true),
            _ => (false, false),
        };
        assert_eq!(element.contains("<w:b/>"), bold, "{id}: {element}");
        assert_eq!(element.contains("<w:b w:val=\"0\"/>"), !bold, "{id}: {element}");
        assert_eq!(element.contains("<w:i/>"), italic, "{id}: {element}");
        assert_eq!(element.contains("<w:i w:val=\"0\"/>"), !italic, "{id}: {element}");
    }
    assert!(styles.contains("w:eastAsia=\"Novel Body Face\""));
    assert!(styles.contains("w:bCs") && styles.contains("w:iCs"));
    let document = xml(&output);
    assert!(document.contains("w:w=\"11906\" w:h=\"16838\""), "{document}");
    assert!(document.contains("w:top=\"1134\" w:right=\"1134\" w:bottom=\"1134\" w:left=\"1134\""), "{document}");
    assert!(document.contains("w:line=\"360\""));
    let paragraph_for = |text: &str| {
        let at = document.find(text).unwrap();
        let start = document[..at].rfind("<w:p>").unwrap();
        let end = document[at..].find("</w:p>").unwrap() + at + "</w:p>".len();
        &document[start..end]
    };
    // Profile line spacing affects source body paragraphs only; source headings and generated
    // title paragraphs retain their former 1.15 spacing.
    assert!(paragraph_for("Scene ordering uses").contains("w:line=\"360\""));
    assert!(paragraph_for("Nested heading level three").contains("w:line=\"276\""));
    for text in ["Synthetic 世界", "Chapter 0 — First / الفصل الأول", "Late-position scene"] {
        let paragraph = paragraph_for(text);
        assert!(paragraph.contains("w:line=\"276\""), "generated title lost its prior spacing: {paragraph}");
    }
    // Explicit Arabic marks still emit complex-script properties after style inheritance.
    let source = xml(&output);
    assert!(source.contains("<w:b/>") && source.contains("<w:bCs/>") && source.contains("<w:i/>") && source.contains("<w:iCs/>"));
    let reread = wordcraft_docx::read(&output).unwrap();
    let texts = paragraph_texts(&reread);
    assert!(texts.iter().any(|text| text == "سلامٌ مِمِمِمُنسَّق"));
    assert!(texts.iter().any(|text| text.contains("Before\nafter hard break")));
    assert!(texts.iter().any(|text| text.contains("简体中文故事")));
}

#[test]
fn invalid_direct_profiles_are_refused_before_export() {
    let bundle = fixture();
    let mut profile = ExportProfile::default();
    profile.margin_mm = f32::NAN;
    assert!(export_manuscript_with_profile(&bundle, &profile).is_err());
    let mut profile = ExportProfile::default();
    profile.body.font = "Font\u{0001}Name".into();
    assert!(manuscript_document_with_profile(&bundle, &profile).is_err());
    let mut bundle = fixture();
    bundle.scenes[0].scene.document["content"][0]["attrs"]["lineSpacing"] = json!(1.2);
    let mut profile = ExportProfile::default();
    profile.line_spacing = "2".into();
    assert!(export_manuscript_with_profile(&bundle, &profile).unwrap_err().contains("line spacing"));
}
