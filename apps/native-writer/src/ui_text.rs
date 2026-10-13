//! Native interface text, kept separate from manuscript data and route IDs.
//!
//! The current shipped catalogue is English. Additional UI locales can be added
//! deliberately; an unavailable locale returns English; a future locale control must show its notice. This
//! module never reads system settings, downloads text, or translates author data.
include!("ui_text_catalog.rs");

pub(crate) const LOCALE: &str = "en";

/// A catalogue lookup is exact and case-sensitive. Pass a stable interface key,
/// never a scene title, record name, language ID, command name or saved text.
pub(crate) fn get(key: &str) -> Option<&'static str> {
    ENGLISH
        .binary_search_by_key(&key, |(key, _)| *key)
        .ok()
        .map(|index| ENGLISH[index].1)
}

pub(crate) fn text(key: &str) -> &'static str {
    text_for(LOCALE, key)
}

/// Other locales are not claimed as translated. Labels remain usable offline.
pub(crate) fn text_for(_locale: &str, key: &str) -> &'static str {
    get(key).unwrap_or("Interface text unavailable")
}

#[cfg(test)] // There is no production UI-locale selector in this English slice.
pub(crate) fn locale_notice(locale: &str) -> Option<&'static str> {
    (locale != LOCALE).then(|| text("notice.locale_unavailable"))
}

#[cfg(test)]
#[path = "ui_text_tests.rs"]
mod tests;
