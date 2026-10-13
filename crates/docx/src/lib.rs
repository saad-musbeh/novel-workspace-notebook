//! Saved manuscript snapshots -> DOCX. No editor, storage mutation or font assets.
#![forbid(unsafe_code)]

use novel_storage::{ExportProfile, ExportStyle, ProjectBundle};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use wordcraft_doc::{para_block, Document, InlineObject, Paragraph};
use wordcraft_doc::para::Run;
use wordcraft_doc::props::{CharProps, LineSpacing, ParaProps, Underline};

pub const FILE_LIMIT: usize = 64 * 1024 * 1024;
const RECORD_LIMIT: usize = 20_000;
const PARAGRAPH_LIMIT: usize = 100_000;
const RUN_LIMIT: usize = 500_000;
const OBJECT_LIMIT: usize = 500_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDocument {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    content: Vec<SourceBlock>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    attrs: Attributes,
    #[serde(default)]
    content: Vec<SourceInline>,
}
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Attributes {
    dir: Option<String>,
    lang: Option<String>,
    level: Option<u8>,
    line_spacing: Option<f32>,
}
#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
enum SourceInline {
    #[serde(rename = "text")]
    Text { text: String, #[serde(default)] marks: Vec<Mark> },
    #[serde(rename = "hardBreak")]
    HardBreak,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mark {
    #[serde(rename = "type")]
    kind: String,
}

fn check_text(text: &str) -> Result<(), String> {
    if text.chars().any(|c| !matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) {
        return Err("DOCX cannot represent a control character in this manuscript. The source was preserved; use a complete project archive to retain it.".into());
    }
    Ok(())
}

fn auto_rtl(text: &str) -> bool {
    use unicode_bidi::BidiClass::{AL, L, R};
    text.chars().find_map(|c| match unicode_bidi::bidi_class(c) {
        R | AL => Some(true), L => Some(false), _ => None,
    }).unwrap_or(false)
}

fn paragraph_props(style: &str, rtl: bool, spacing: f32) -> ParaProps {
    ParaProps {
        style: Some(style.into()), bidi: Some(rtl),
        space_before: Some(0.0), space_after: Some(0.0),
        line_spacing: Some(LineSpacing::Multiple(spacing)),
        ..Default::default()
    }
}

fn append_run(p: &mut Paragraph, text: &str, props: CharProps) -> Result<(), String> {
    check_text(text)?;
    // U+FFFC can be literal manuscript text. WordCraft reserves it for objects;
    // its opaque-text fallback emits that exact character with the run's marks.
    let literals = text.chars().filter(|c| *c == wordcraft_doc::para::OBJ).count();
    if p.objects.len().saturating_add(literals) > OBJECT_LIMIT {
        return Err("The manuscript exceeds the DOCX literal-object limit. The source was preserved.".into());
    }
    p.objects.extend((0..literals).map(|_| InlineObject::Opaque {
        format: "novel-literal-text".into(), xml: String::new(), text: wordcraft_doc::para::OBJ.to_string(),
    }));
    if !text.is_empty() {
        p.text.push_str(text);
        p.runs.push(Run { len: text.len(), props });
    }
    Ok(())
}

fn title_paragraph(text: &str, style: &str) -> Result<Paragraph, String> {
    let mut p = Paragraph::new();
    p.props = paragraph_props(style, auto_rtl(text), 1.15);
    p.props.keep_next = Some(true);
    p.mark = CharProps { lang: Some("und".into()), ..Default::default() };
    let props = p.mark.clone();
    append_run(&mut p, text, props)?;
    Ok(p)
}

fn push_paragraph(document: &mut Document, objects: &mut usize, paragraph: Paragraph) -> Result<(), String> {
    *objects = objects.saturating_add(paragraph.objects.len());
    if *objects > OBJECT_LIMIT {
        return Err("The manuscript exceeds the DOCX literal-object limit. The source was preserved.".into());
    }
    document.body.push(para_block(paragraph));
    Ok(())
}

fn profile_spacing(value: &str) -> Option<f32> {
    match value {
        "1" => Some(1.0),
        "1.15" => Some(1.15),
        "1.5" => Some(1.5),
        "2" => Some(2.0),
        _ => None,
    }
}

fn source_paragraph(block: SourceBlock, profile: &ExportProfile) -> Result<Paragraph, String> {
    let language = block.attrs.lang.as_deref().unwrap_or("en");
    if !["en", "ar", "zh-Hans", "zh-Hant", "und"].contains(&language) {
        return Err("Unsupported manuscript language for DOCX export.".into());
    }
    let source_spacing = block.attrs.line_spacing.unwrap_or(1.15);
    if ![1.0, 1.15, 1.5, 2.0].contains(&source_spacing) {
        return Err("Unsupported manuscript line spacing for DOCX export.".into());
    }
    // Profile body spacing is deliberately limited to source paragraphs. Source headings
    // and the generated document/chapter/scene titles keep their established spacing.
    let spacing = if block.kind == "paragraph" {
        profile_spacing(&profile.line_spacing).unwrap_or(source_spacing)
    } else {
        source_spacing
    };
    if ![1.0, 1.15, 1.5, 2.0].contains(&spacing) {
        return Err("Unsupported manuscript line spacing for DOCX export.".into());
    }
    let style = match block.kind.as_str() {
        "paragraph" if block.attrs.level.is_none() => "Normal".to_string(),
        "heading" => {
            let level = block.attrs.level.unwrap_or(2);
            if !(1..=6).contains(&level) { return Err("Unsupported manuscript heading level.".into()); }
            format!("Heading{level}")
        }
        _ => return Err("Unsupported manuscript paragraph for DOCX export.".into()),
    };
    let mut p = Paragraph::new();
    p.mark = CharProps { lang: Some(language.into()), ..Default::default() };
    for node in block.content {
        match node {
            SourceInline::HardBreak => { let props = p.mark.clone(); append_run(&mut p, "\n", props)?; }
            SourceInline::Text { text, marks } => {
                let mut props = p.mark.clone();
                let mut seen = HashSet::new();
                for mark in marks {
                    if !seen.insert(mark.kind.clone()) { return Err("Duplicate manuscript formatting.".into()); }
                    match mark.kind.as_str() {
                        "bold" => props.bold = Some(true),
                        "italic" => props.italic = Some(true),
                        "underline" => props.underline = Some(Underline::Single),
                        _ => return Err("Unsupported manuscript formatting for DOCX export.".into()),
                    }
                }
                append_run(&mut p, &text, props)?;
            }
        }
    }
    let rtl = match block.attrs.dir.as_deref().unwrap_or("auto") {
        "auto" => auto_rtl(&p.text), "rtl" => true, "ltr" => false,
        _ => return Err("Unsupported manuscript direction for DOCX export.".into()),
    };
    p.props = paragraph_props(&style, rtl, spacing);
    if block.kind == "heading" { p.props.keep_next = Some(true); }
    Ok(p)
}

/// Build a disposable WordCraft document; the saved source and its ids stay intact.
pub fn manuscript_document(bundle: &ProjectBundle) -> Result<Document, String> {
    manuscript_document_with_profile(bundle, &ExportProfile::default())
}

fn apply_style(style: &mut wordcraft_doc::styles::Style, profile: &ExportStyle) {
    style.chr.font = Some(profile.font.clone());
    style.chr.size = Some(profile.size);
    style.chr.bold = Some(profile.bold);
    style.chr.italic = Some(profile.italic);
}

fn apply_profile(document: &mut Document, profile: &ExportProfile) {
    let section = &mut document.last_section;
    let (width, height) = if profile.page_size == "a4" {
        (210.0 / 25.4 * 72.0, 297.0 / 25.4 * 72.0)
    } else {
        (612.0, 792.0)
    };
    let margin = profile.margin_mm / 25.4 * 72.0;
    section.page_w = width;
    section.page_h = height;
    section.margin_top = margin;
    section.margin_bottom = margin;
    section.margin_left = margin;
    section.margin_right = margin;

    let body = &profile.body;
    document.styles.default_chr.font = Some(body.font.clone());
    document.styles.default_chr.size = Some(body.size);
    document.styles.default_chr.bold = Some(body.bold);
    document.styles.default_chr.italic = Some(body.italic);
    for style in &mut document.styles.styles {
        match style.id.as_str() {
            "Normal" => apply_style(style, body),
            "Title" => apply_style(style, &profile.title),
            "Heading1" => apply_style(style, &profile.chapter),
            "Heading2" => apply_style(style, &profile.scene),
            _ => {}
        }
    }
}

/// Build a disposable WordCraft document using a validated saved-manuscript profile.
pub fn manuscript_document_with_profile(bundle: &ProjectBundle, profile: &ExportProfile) -> Result<Document, String> {
    profile.validate()?;
    if bundle.chapters.len() > RECORD_LIMIT || bundle.scenes.len() > RECORD_LIMIT {
        return Err("The manuscript exceeds the DOCX export record limit.".into());
    }
    if bundle.chapter_numbering_conflicted {
        return Err("Resolve conflicting chapter numbering in Recovery before exporting a Word manuscript.".into());
    }
    if ![0, 1].contains(&bundle.chapter_number_start) {
        return Err("Unsupported starting chapter number.".into());
    }
    let mut chapter_ids = HashSet::new();
    let mut scene_ids = HashSet::new();
    let mut chapters: Vec<_> = bundle.chapters.iter().collect();
    for chapter in &chapters {
        if chapter.id.is_empty() || chapter.project_id != bundle.project.id || chapter.position < 0 || !chapter_ids.insert(chapter.id.as_str()) {
            return Err("The manuscript has an invalid chapter identity or placement.".into());
        }
    }
    let mut scenes_by_chapter: HashMap<&str, Vec<_>> = HashMap::new();
    let mut source_bytes = bundle.project.title.len();
    for scene in &bundle.scenes {
        if scene.id.is_empty() || scene.revision_id.is_empty() || scene.position < 0 || !scene_ids.insert(scene.id.as_str()) || !chapter_ids.contains(scene.chapter_id.as_str()) {
            return Err("The manuscript has an invalid scene identity or placement.".into());
        }
        novel_storage::validate(&scene.scene)?;
        source_bytes = source_bytes.saturating_add(serde_json::to_vec(&scene.scene).map_err(|_| "Invalid manuscript scene.")?.len());
        if source_bytes > FILE_LIMIT { return Err("The manuscript exceeds the 64 MiB DOCX source limit.".into()); }
        scenes_by_chapter.entry(&scene.chapter_id).or_default().push(scene);
    }
    chapters.sort_by_key(|c| (c.position, c.id.as_str()));
    for scenes in scenes_by_chapter.values_mut() { scenes.sort_by_key(|s| (s.position, s.id.as_str())); }
    let mut document = Document::new();
    document.body.clear();
    document.core.title = bundle.project.title.clone();
    document.core.creator = "".into();
    // Neutral manuscript formatting; font binaries are neither copied nor embedded.
    for style in &mut document.styles.styles {
        style.para.space_before = Some(0.0);
        style.para.space_after = Some(0.0);
        style.chr.color = None;
    }
    apply_profile(&mut document, profile);
    let mut objects = 0usize;
    push_paragraph(&mut document, &mut objects, title_paragraph(&bundle.project.title, "Title")?)?;
    let mut paragraphs = 1;
    let mut runs = 0usize;
    for (index, chapter) in chapters.iter().enumerate() {
        let number = bundle.chapter_number_start + index as i64;
        let mut heading = title_paragraph(&format!("Chapter {number} — {}", chapter.title), "Heading1")?;
        if index > 0 { heading.props.page_break_before = Some(true); }
        push_paragraph(&mut document, &mut objects, heading)?;
        paragraphs += 1;
        for scene in scenes_by_chapter.get(chapter.id.as_str()).into_iter().flatten() {
            push_paragraph(&mut document, &mut objects, title_paragraph(&scene.scene.title, "Heading2")?)?;
            paragraphs += 1;
            let source: SourceDocument = serde_json::from_value(scene.scene.document.clone()).map_err(|_| "This scene contains unsupported document metadata. DOCX export stopped without changing the source.")?;
            if source.kind != "doc" { return Err("Invalid manuscript document root.".into()); }
            paragraphs += source.content.len();
            runs = runs.saturating_add(source.content.iter().map(|b| b.content.len()).sum::<usize>());
            if paragraphs > PARAGRAPH_LIMIT || runs > RUN_LIMIT { return Err("The manuscript exceeds the DOCX paragraph or run limit.".into()); }
            for block in source.content {
                let paragraph = source_paragraph(block, profile)?;
                push_paragraph(&mut document, &mut objects, paragraph)?;
            }
        }
    }
    Ok(document)
}

pub fn export_manuscript(bundle: &ProjectBundle) -> Result<Vec<u8>, String> {
    export_manuscript_with_profile(bundle, &ExportProfile::default())
}

pub fn export_manuscript_with_profile(bundle: &ProjectBundle, profile: &ExportProfile) -> Result<Vec<u8>, String> {
    let document = manuscript_document_with_profile(bundle, profile)?;
    let bytes = wordcraft_docx::write(&document).map_err(|_| "Could not produce the Word manuscript. The source was preserved.")?;
    if bytes.len() > FILE_LIMIT { return Err("The DOCX exceeds the 64 MiB file limit.".into()); }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
