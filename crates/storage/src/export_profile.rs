use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportStyle {
    pub font: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExportProfile {
    pub schema_version: u32,
    pub page_size: String,
    pub margin_mm: f32,
    pub line_spacing: String,
    pub title: ExportStyle,
    pub chapter: ExportStyle,
    pub scene: ExportStyle,
    pub body: ExportStyle,
}

impl Default for ExportProfile {
    fn default() -> Self {
        let style = |font: &str, size| ExportStyle {
            font: font.into(), size, bold: false, italic: false,
        };
        Self {
            schema_version: 1, page_size: "letter".into(), margin_mm: 25.4,
            line_spacing: "source".into(),
            title: style("Aptos Display", 28.0),
            chapter: style("Aptos Display", 20.0),
            scene: style("Aptos Display", 16.0),
            body: style("Aptos", 12.0),
        }
    }
}

impl ExportProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("Unsupported Word export style version.".into());
        }
        if !["letter", "a4"].contains(&self.page_size.as_str()) {
            return Err("Choose Letter or A4 paper for Word export.".into());
        }
        if !self.margin_mm.is_finite() || !(10.0..=40.0).contains(&self.margin_mm) {
            return Err("Word export margins must be between 10 and 40 mm.".into());
        }
        if !["source", "1", "1.15", "1.5", "2"].contains(&self.line_spacing.as_str()) {
            return Err("Unsupported Word export body spacing.".into());
        }
        for style in [&self.title, &self.chapter, &self.scene, &self.body] {
            if style.font.trim() != style.font || style.font.is_empty()
                || style.font.chars().count() > 80
                || style.font.chars().any(|ch| ch.is_control() || matches!(ch, '\u{fffe}' | '\u{ffff}'))
            {
                return Err("Word export font names must contain 1–80 characters without controls or surrounding spaces.".into());
            }
            if !style.size.is_finite() || !(8.0..=72.0).contains(&style.size)
                || (style.size * 2.0).fract() != 0.0
            {
                return Err("Word export font sizes must be 8–72 points in half-point steps.".into());
            }
        }
        Ok(())
    }
}
