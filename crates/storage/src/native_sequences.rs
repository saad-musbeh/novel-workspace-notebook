//! Bounded formatting-only sequences stored additively in project `settings`.
//! Root must export this module and call `from_project_settings` in validate_data's
//! `settings` arm, so imports, sync and every settings writer share validation.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const NATIVE_SEQUENCE_KEY: &str = "nativeCommandSequences";
pub const SEQUENCE_SLOTS: u8 = 8;
pub const MAX_SEQUENCE_COMMANDS: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SequenceFormat {
    Bold,
    Italic,
    Underline,
    ClearFormatting,
    Ltr,
    Rtl,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SequenceStyle {
    Paragraph,
    Heading,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SequenceLanguage {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "ar")]
    Arabic,
    #[serde(rename = "zh-Hans")]
    ChineseSimplified,
    #[serde(rename = "zh-Hant")]
    ChineseTraditional,
    #[serde(rename = "und")]
    Undetermined,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SequenceDirection {
    Auto,
    Ltr,
    Rtl,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum NativeSequenceAction {
    Format {
        command: SequenceFormat,
    },
    Spacing {
        value: f64,
    },
    Paragraph {
        style: SequenceStyle,
        language: SequenceLanguage,
        direction: SequenceDirection,
    },
}
impl NativeSequenceAction {
    pub fn validate(&self) -> Result<(), String> {
        if let Self::Spacing { value } = self {
            if ![1., 1.15, 1.5, 2.].contains(value) {
                return Err("Use line spacing 1, 1.15, 1.5 or 2.".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeCommandSequence {
    pub slot: u8,
    pub name: String,
    pub actions: Vec<NativeSequenceAction>,
}
impl NativeCommandSequence {
    pub fn validate_name(slot: u8, name: &str) -> Result<(), String> {
        if !(1..=SEQUENCE_SLOTS).contains(&slot) {
            return Err("Choose a command sequence slot from 1 to 8.".into());
        }
        // Match the retained interface's JavaScript string length limit.
        if name.trim().is_empty() || name.encode_utf16().count() > 80 {
            return Err("Use a sequence name of at most 80 UTF-16 units.".into());
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        Self::validate_name(self.slot, &self.name)?;
        if self.actions.is_empty() || self.actions.len() > MAX_SEQUENCE_COMMANDS {
            return Err("A sequence needs between one and 32 commands.".into());
        }
        for action in &self.actions {
            action.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeSequenceSettings {
    pub schema_version: u32,
    pub sequences: Vec<NativeCommandSequence>,
}
impl Default for NativeSequenceSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            sequences: vec![],
        }
    }
}
impl NativeSequenceSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 || self.sequences.len() > SEQUENCE_SLOTS as usize {
            return Err("Unsupported native command sequences version or slot count.".into());
        }
        let mut seen = BTreeSet::new();
        for sequence in &self.sequences {
            sequence.validate()?;
            if !seen.insert(sequence.slot) {
                return Err("A command sequence slot is repeated.".into());
            }
        }
        Ok(())
    }
    pub fn from_project_settings(value: &Value) -> Result<Self, String> {
        let Some(sequences) = value.get(NATIVE_SEQUENCE_KEY) else {
            return Ok(Self::default());
        };
        let parsed: Self = serde_json::from_value(sequences.clone())
            .map_err(|_| "Invalid native command sequence fields.")?;
        parsed.validate()?;
        Ok(parsed)
    }
    /// Preserve every unrelated setting; the caller must CAS the captured head.
    pub fn merge_into(&self, value: &Value) -> Result<Value, String> {
        self.validate()?;
        if !value.is_object() {
            return Err("Project settings must be an object.".into());
        }
        let mut value = value.clone();
        value[NATIVE_SEQUENCE_KEY] = serde_json::to_value(self).map_err(|e| e.to_string())?;
        Ok(value)
    }
    pub fn sequence(&self, slot: u8) -> Option<&NativeCommandSequence> {
        self.sequences.iter().find(|sequence| sequence.slot == slot)
    }
}
