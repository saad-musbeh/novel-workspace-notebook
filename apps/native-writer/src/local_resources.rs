//! Bounded local resource formats. No chooser, network, database or registration.
use novel_storage::ProjectAsset;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
pub(super) type Result<T> = std::result::Result<T, String>;
pub(super) const PACK_LIMIT: usize = 2_000_000;
pub(super) const FONT_LIMIT: usize = 16 * 1024 * 1024;
pub(super) fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn string<'a>(v: &'a Value, key: &str, max: usize, required: bool) -> Result<&'a str> {
    v[key]
        .as_str()
        .filter(|s| s.encode_utf16().count() <= max && (!required || !s.trim().is_empty()))
        .ok_or_else(|| format!("Invalid resource {key}."))
}
fn fields(v: &Value, required: &[&str], optional: &[&str]) -> Result<()> {
    let o = v.as_object().ok_or("Expected a resource object.")?;
    if required.iter().any(|k| !o.contains_key(*k))
        || o.keys()
            .any(|k| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err("Missing or unsupported resource fields.".into());
    }
    Ok(())
}
fn array<'a>(v: &'a Value, key: &str, max: usize) -> Result<&'a Vec<Value>> {
    v[key]
        .as_array()
        .filter(|a| a.len() <= max)
        .ok_or_else(|| format!("Invalid resource {key} collection."))
}
fn hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn identity(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.as_bytes()[0].is_ascii_alphanumeric()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// JavaScript canonicalJson sorts object keys by UTF-16 code units. The strict
/// schema uses ASCII keys and integral schemaVersion; arbitrary floats are refused.
pub(super) fn canonical_json(v: &Value) -> Result<String> {
    match v {
        Value::Object(o) => {
            let mut keys = o.keys().collect::<Vec<_>>();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            let mut parts = Vec::new();
            for k in keys {
                parts.push(format!(
                    "{}:{}",
                    serde_json::to_string(k).map_err(|e| e.to_string())?,
                    canonical_json(&o[k])?
                ));
            }
            Ok(format!("{{{}}}", parts.join(",")))
        }
        Value::Array(a) => Ok(format!(
            "[{}]",
            a.iter()
                .map(canonical_json)
                .collect::<Result<Vec<_>>>()?
                .join(",")
        )),
        Value::Number(n) if !n.is_u64() && !n.is_i64() => {
            Err("Pack checksums require integral schema values.".into())
        }
        _ => serde_json::to_string(v).map_err(|e| e.to_string()),
    }
}
pub(super) fn pack_checksum(pack: &Value) -> Result<String> {
    let mut payload = pack.clone();
    payload
        .as_object_mut()
        .ok_or("Expected a pack object.")?
        .remove("checksum");
    Ok(checksum(canonical_json(&payload)?.as_bytes()))
}
pub(super) fn validate_pack(pack: &Value) -> Result<()> {
    fields(
        pack,
        &[
            "schemaVersion",
            "id",
            "name",
            "language",
            "variant",
            "version",
            "source",
            "license",
            "attribution",
            "operations",
            "checksum",
            "entries",
        ],
        &[],
    )?;
    if pack["schemaVersion"].as_u64() != Some(1) {
        return Err("Unsupported language-pack version.".into());
    }
    for k in [
        "id",
        "name",
        "language",
        "variant",
        "version",
        "source",
        "license",
        "attribution",
    ] {
        string(pack, k, if k == "attribution" { 5000 } else { 500 }, true)?;
    }
    if !identity(pack["id"].as_str().unwrap(), 100) {
        return Err("Invalid pack identity.".into());
    }
    let operations = array(pack, "operations", 3)?;
    let mut ops = HashSet::new();
    if operations.is_empty() {
        return Err("A pack must declare its operations.".into());
    }
    for op in operations {
        let op = op
            .as_str()
            .filter(|s| ["dictionary", "synonyms", "glossary"].contains(s))
            .ok_or("Unsupported pack operation.")?;
        if !ops.insert(op) {
            return Err("Duplicate pack operation.".into());
        }
    }
    let mut terms = HashSet::new();
    for e in array(pack, "entries", 10_000)? {
        fields(e, &["term", "senses"], &["notes"])?;
        let term = string(e, "term", 200, true)?;
        if !terms.insert(term) {
            return Err("Duplicate pack term.".into());
        }
        if e.get("notes").is_some() {
            string(e, "notes", 2000, false)?;
        }
        let senses = array(e, "senses", 100)?;
        if senses.is_empty() {
            return Err("A term needs bounded senses.".into());
        }
        let mut ids = HashSet::new();
        for sense in senses {
            fields(
                sense,
                &[
                    "id",
                    "definition",
                    "partOfSpeech",
                    "synonyms",
                    "related",
                    "translations",
                ],
                &[],
            )?;
            if !ids.insert(string(sense, "id", 200, true)?) {
                return Err("Duplicate sense identity.".into());
            }
            string(sense, "definition", 3000, true)?;
            string(sense, "partOfSpeech", 200, true)?;
            for k in ["synonyms", "related"] {
                for word in array(sense, k, 100)? {
                    if !word
                        .as_str()
                        .is_some_and(|s| !s.trim().is_empty() && s.encode_utf16().count() <= 200)
                    {
                        return Err("Invalid lexical alternative.".into());
                    }
                }
            }
            for t in array(sense, "translations", 100)? {
                fields(t, &["language", "text"], &[])?;
                string(t, "language", 200, true)?;
                string(t, "text", 200, true)?;
            }
            if !ops.contains("synonyms") && !sense["synonyms"].as_array().unwrap().is_empty() {
                return Err("Synonyms are not declared by this pack.".into());
            }
        }
    }
    let digest = string(pack, "checksum", 64, true)?;
    if !hex(digest) || pack_checksum(pack)? != digest {
        return Err("Pack checksum mismatch. Nothing was imported.".into());
    }
    if serde_json::to_vec(pack).map_err(|e| e.to_string())?.len() > PACK_LIMIT {
        return Err("Pack exceeds the 2 MB limit.".into());
    }
    Ok(())
}
pub(super) fn import_pack(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > PACK_LIMIT {
        return Err("Pack exceeds the 2 MB import limit.".into());
    }
    let pack = serde_json::from_slice(bytes)
        .map_err(|_| "Choose a valid UTF-8 language-pack JSON file.")?;
    validate_pack(&pack)?;
    Ok(pack)
}
pub(super) fn validate_pack_document(doc: &Value) -> Result<()> {
    if doc["schemaVersion"].as_u64() != Some(1)
        || serde_json::to_vec(doc).map_err(|e| e.to_string())?.len() > 8_000_000
    {
        return Err("Invalid or oversized language-pack document.".into());
    }
    let mut ids = HashSet::new();
    for p in array(doc, "packs", 20)? {
        validate_pack(p)?;
        if !ids.insert(p["id"].as_str().unwrap()) {
            return Err("Only one active version per pack identity is allowed.".into());
        }
    }
    Ok(())
}
/// Complete, checked historical payloads establish immutable (id,version) content.
#[derive(Clone, Debug, Default)]
pub(super) struct PackVersions(HashMap<(String, String), String>);
impl PackVersions {
    pub fn observe(&mut self, doc: &Value) -> Result<()> {
        validate_pack_document(doc)?;
        for pack in doc["packs"].as_array().unwrap() {
            let key = (
                pack["id"].as_str().unwrap().to_owned(),
                pack["version"].as_str().unwrap().to_owned(),
            );
            if self
                .0
                .get(&key)
                .is_some_and(|v| Some(v.as_str()) != pack["checksum"].as_str())
            {
                return Err(
                    "A retained pack identity/version has different immutable content.".into(),
                );
            }
            self.0
                .insert(key, pack["checksum"].as_str().unwrap().to_owned());
        }
        Ok(())
    }
    pub fn install(&self, doc: &Value, incoming: &Value) -> Result<Value> {
        validate_pack_document(doc)?;
        validate_pack(incoming)?;
        let key = (
            incoming["id"].as_str().unwrap().to_owned(),
            incoming["version"].as_str().unwrap().to_owned(),
        );
        if self
            .0
            .get(&key)
            .is_some_and(|v| Some(v.as_str()) != incoming["checksum"].as_str())
        {
            return Err(
                "This pack version already has different retained content. Use a new version."
                    .into(),
            );
        }
        if doc["packs"].as_array().unwrap().iter().any(|p| {
            p["id"] == incoming["id"] && p["version"] == incoming["version"] && p != incoming
        }) {
            return Err("This active pack version has different content.".into());
        }
        let mut next = doc.clone();
        let packs = next["packs"].as_array_mut().unwrap();
        if let Some(index) = packs.iter().position(|p| p["id"] == incoming["id"]) {
            packs[index] = incoming.clone();
        } else {
            packs.push(incoming.clone());
        }
        validate_pack_document(&next)?;
        Ok(next)
    }
}
pub(super) fn read_selected(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > limit as u64 {
        return Err("Choose a regular local file within the import size limit.".into());
    }
    let f = File::open(path).map_err(|e| e.to_string())?;
    let m = f.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.len() > limit as u64 {
        return Err("The chosen local file changed or exceeds its limit.".into());
    }
    let mut bytes = Vec::new();
    f.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("The chosen file exceeds its import limit.".into());
    }
    Ok(bytes)
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FontInfo {
    pub format: &'static str,
    pub family: String,
}
fn u16be(b: &[u8], at: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(
        b.get(at..at + 2)
            .ok_or("Font field exceeds its bounds.")?
            .try_into()
            .unwrap(),
    ))
}
fn u32be(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        b.get(at..at + 4)
            .ok_or("Font field exceeds its bounds.")?
            .try_into()
            .unwrap(),
    ))
}
fn table_sum(bytes: &[u8], head: bool) -> u32 {
    let mut sum = 0u32;
    for (i, part) in bytes.chunks(4).enumerate() {
        let mut word = [0; 4];
        word[..part.len()].copy_from_slice(part);
        if head && i == 2 {
            word = [0; 4];
        }
        sum = sum.wrapping_add(u32::from_be_bytes(word));
    }
    sum
}
fn cff_index(data: &[u8], at: usize, wide: bool) -> Result<(usize, usize)> {
    let count = if wide {
        u32be(data, at)? as usize
    } else {
        u16be(data, at)? as usize
    };
    let start = at + if wide { 4 } else { 2 };
    if count == 0 {
        return Ok((start, 0));
    }
    if count > 65535 {
        return Err("CFF index exceeds its bounded object count.".into());
    }
    let width = *data.get(start).ok_or("Missing CFF index offset size.")? as usize;
    if !(1..=4).contains(&width) {
        return Err("Invalid CFF index offset size.".into());
    }
    let offsets = start + 1;
    let content = offsets + (count + 1) * width;
    if content > data.len() {
        return Err("CFF index offsets exceed the outline table.".into());
    }
    let mut previous = 1usize;
    for i in 0..=count {
        let raw = data
            .get(offsets + i * width..offsets + (i + 1) * width)
            .ok_or("Invalid CFF index offset.")?;
        let offset = raw.iter().fold(0usize, |n, b| (n << 8) | (*b as usize));
        if offset == 0
            || i == 0 && offset != 1
            || offset < previous
            || content
                .checked_add(offset - 1)
                .is_none_or(|n| n > data.len())
        {
            return Err("CFF index data exceeds its table bounds.".into());
        }
        previous = offset;
    }
    Ok((content + previous - 1, count))
}
fn inspect_cff(data: &[u8], version2: bool) -> Result<()> {
    if data.len() < if version2 { 5 } else { 4 }
        || data[0] != if version2 { 2 } else { 1 }
        || data[1] != 0
    {
        return Err("Invalid CFF outline header.".into());
    }
    let header = data[2] as usize;
    if header < if version2 { 5 } else { 4 } || header > data.len() {
        return Err("Invalid CFF header bounds.".into());
    }
    if version2 {
        let length = u16be(data, 3)? as usize;
        let end = header + length;
        if end > data.len() {
            return Err("CFF2 top dictionary exceeds its table.".into());
        }
        cff_index(data, end, true)?;
    } else {
        if !(1..=4).contains(&data[3]) {
            return Err("Invalid CFF header offset size.".into());
        }
        let (next, count) = cff_index(data, header, false)?;
        if count != 1 {
            return Err("A single-font CFF name index is required.".into());
        }
        let (next, count) = cff_index(data, next, false)?;
        if count != 1 {
            return Err("A single-font CFF dictionary index is required.".into());
        }
        let (next, _) = cff_index(data, next, false)?;
        cff_index(data, next, false)?;
    }
    Ok(())
}
/// Structural SFNT validation precedes the helper's independent FreeType/family
/// validation. This does not prove glyph coverage, shaping, or legal entitlement.
pub(super) fn inspect_font(bytes: &[u8]) -> Result<FontInfo> {
    if bytes.len() < 12 || bytes.len() > FONT_LIMIT {
        return Err("Choose a single-font TTF or OTF no larger than 16 MiB.".into());
    }
    let format = match &bytes[..4] {
        b"\0\x01\0\0" | b"true" => "ttf",
        b"OTTO" => "otf",
        _ => {
            return Err(
                "Only single-font TTF and OTF are supported; collections/WOFF are refused.".into(),
            )
        }
    };
    let count = u16be(bytes, 4)? as usize;
    let end = 12 + 16 * count;
    if count == 0 || count > 256 || end > bytes.len() {
        return Err("Invalid bounded font directory.".into());
    }
    let mut tables = HashMap::new();
    let mut ranges = Vec::new();
    for i in 0..count {
        let at = 12 + 16 * i;
        let tag: [u8; 4] = bytes[at..at + 4].try_into().unwrap();
        if !tag.iter().all(|b| (0x20..=0x7e).contains(b)) {
            return Err("Invalid font table tag.".into());
        }
        let offset = u32be(bytes, at + 8)? as usize;
        let len = u32be(bytes, at + 12)? as usize;
        let table = bytes
            .get(offset..offset.checked_add(len).ok_or("Font table overflow.")?)
            .ok_or("Font table exceeds its file bounds.")?;
        if offset < end || offset % 4 != 0 || tables.insert(tag, table).is_some() {
            return Err("Duplicate or invalid font table offset.".into());
        }
        if table_sum(table, tag == *b"head") != u32be(bytes, at + 4)? {
            return Err("Font table checksum failed.".into());
        }
        if len > 0 {
            ranges.push((offset, offset + len));
        }
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|r| r[0].1 > r[1].0) {
        return Err("Font tables overlap.".into());
    }
    let get = |tag: &[u8; 4], min: usize| -> Result<&[u8]> {
        tables
            .get(tag)
            .copied()
            .filter(|b| b.len() >= min)
            .ok_or_else(|| {
                format!(
                    "Missing or incomplete font table {}.",
                    String::from_utf8_lossy(tag)
                )
            })
    };
    let head = get(b"head", 54)?;
    if u32be(head, 0)? != 0x10000
        || u32be(head, 12)? != 0x5f0f3cf5
        || !(16..=16384).contains(&u16be(head, 18)?)
        || table_sum(bytes, false) != 0xb1b0afba
    {
        return Err("Invalid SFNT head or whole-font checksum.".into());
    }
    let hhea = get(b"hhea", 36)?;
    let maxp = get(b"maxp", 6)?;
    if u32be(hhea, 0)? != 0x10000
        || u32be(maxp, 0)? != if format == "ttf" { 0x10000 } else { 0x5000 }
    {
        return Err("Unsupported font metric/glyph table version.".into());
    }
    let glyphs = u16be(maxp, 4)? as usize;
    let metrics = u16be(hhea, 34)? as usize;
    if glyphs == 0 || metrics == 0 || metrics > glyphs {
        return Err("Invalid bounded font glyph/metric count.".into());
    }
    get(b"hmtx", 4 * metrics + 2 * (glyphs - metrics))?;
    if format == "ttf" {
        get(b"maxp", 32)?;
        let loca = get(
            b"loca",
            if u16be(head, 50)? == 0 {
                2 * (glyphs + 1)
            } else {
                4 * (glyphs + 1)
            },
        )?;
        let glyf = get(b"glyf", 0)?;
        let mut previous = 0;
        for i in 0..=glyphs {
            let offset = match u16be(head, 50)? {
                0 => u16be(loca, 2 * i)? as usize * 2,
                1 => u32be(loca, 4 * i)? as usize,
                _ => return Err("Invalid glyph location format.".into()),
            };
            if offset < previous || offset > glyf.len() {
                return Err("Glyph locations exceed their table bounds.".into());
            }
            previous = offset;
        }
    } else {
        match (
            tables.get(b"CFF " as &[u8; 4]),
            tables.get(b"CFF2" as &[u8; 4]),
        ) {
            (Some(cff), None) => inspect_cff(cff, false)?,
            (None, Some(cff)) => inspect_cff(cff, true)?,
            _ => return Err("OTF must have one CFF or CFF2 outline table.".into()),
        }
    }
    if let Some(os2) = tables.get(b"OS/2" as &[u8; 4]) {
        if os2.len() < 10 || u16be(os2, 8)? & 2 != 0 {
            return Err("The font declares restricted embedding; choose a permitted font.".into());
        }
    }
    let cmap = get(b"cmap", 4)?;
    let records = u16be(cmap, 2)? as usize;
    if u16be(cmap, 0)? != 0 || records == 0 || records > 256 || 4 + 8 * records > cmap.len() {
        return Err("Invalid bounded cmap directory.".into());
    }
    for i in 0..records {
        let offset = u32be(cmap, 4 + 8 * i + 4)? as usize;
        if offset < 4 + 8 * records {
            return Err("Invalid cmap subtable offset.".into());
        }
        let sub = cmap.get(offset..).ok_or("Invalid cmap subtable bounds.")?;
        let format = u16be(sub, 0)?;
        let len = match format {
            0 | 4 | 6 => u16be(sub, 2)? as usize,
            10 | 12 | 13 => u32be(sub, 4)? as usize,
            14 => u32be(sub, 2)? as usize,
            _ => return Err("Unsupported cmap subtable format.".into()),
        };
        let min = match format {
            0 => 262,
            4 => 24,
            6 => 10,
            10 => 20,
            12 | 13 => 16,
            14 => 10,
            _ => unreachable!(),
        };
        if len < min || len > sub.len() {
            return Err("Invalid cmap subtable length.".into());
        }
        let sub = &sub[..len];
        if format == 6 && 10 + 2 * u16be(sub, 8)? as usize > len
            || format == 10
                && 20usize
                    .checked_add(
                        2usize
                            .checked_mul(u32be(sub, 16)? as usize)
                            .ok_or("cmap count overflow.")?,
                    )
                    .is_none_or(|n| n > len)
        {
            return Err("Invalid cmap glyph array bounds.".into());
        }
        if format == 4 {
            let seg = u16be(sub, 6)? as usize;
            if seg == 0 || seg % 2 != 0 || 16 + 4 * seg > len {
                return Err("Invalid cmap segment bounds.".into());
            }
            let segments = seg / 2;
            if u16be(sub, 14 + 2 * segments)? != 0 || u16be(sub, 14 + 2 * (segments - 1))? != 0xffff
            {
                return Err("Invalid cmap sentinel or padding.".into());
            }
            let mut previous = None;
            for i in 0..segments {
                let end = u16be(sub, 14 + 2 * i)? as usize;
                let start = u16be(sub, 16 + 2 * segments + 2 * i)? as usize;
                if start > end || previous.is_some_and(|p| start <= p) {
                    return Err("Invalid cmap segment ordering.".into());
                }
                previous = Some(end);
                let at = 16 + 6 * segments + 2 * i;
                let offset = u16be(sub, at)? as usize;
                if offset != 0
                    && (at + offset < 16 + 8 * segments
                        || at + offset + 2 * (end - start) + 2 > len)
                {
                    return Err("cmap glyph offsets exceed their subtable.".into());
                }
            }
        }
        if matches!(format, 12 | 13)
            && 16usize
                .checked_add(
                    12usize
                        .checked_mul(u32be(sub, 12)? as usize)
                        .ok_or("cmap count overflow.")?,
                )
                .ok_or("cmap length overflow.")?
                > len
        {
            return Err("Invalid cmap group bounds.".into());
        }
        if matches!(format, 12 | 13) {
            let mut previous = None;
            for i in 0..u32be(sub, 12)? as usize {
                let at = 16 + 12 * i;
                let start = u32be(sub, at)?;
                let end = u32be(sub, at + 4)?;
                let glyph = u32be(sub, at + 8)?;
                if start > end
                    || end > 0x10ffff
                    || previous.is_some_and(|p| start <= p)
                    || glyph >= glyphs as u32
                    || format == 12
                        && glyph
                            .checked_add(end - start)
                            .is_none_or(|n| n >= glyphs as u32)
                {
                    return Err("Invalid cmap group ordering or glyph range.".into());
                }
                previous = Some(end);
            }
        }
        if format == 14 {
            let count = u32be(sub, 6)? as usize;
            if 10usize
                .checked_add(
                    11usize
                        .checked_mul(count)
                        .ok_or("cmap variation count overflow.")?,
                )
                .is_none_or(|n| n > len)
            {
                return Err("Invalid cmap variation directory.".into());
            }
            for i in 0..count {
                let at = 10 + 11 * i;
                for (field, width) in [(at + 3, 4usize), (at + 7, 5usize)] {
                    let offset = u32be(sub, field)? as usize;
                    if offset == 0 {
                        continue;
                    }
                    let count = u32be(sub, offset)? as usize;
                    if offset
                        .checked_add(4)
                        .and_then(|n| width.checked_mul(count).and_then(|v| n.checked_add(v)))
                        .is_none_or(|n| n > len)
                    {
                        return Err("Invalid cmap variation data bounds.".into());
                    }
                }
            }
        }
    }
    let name = get(b"name", 6)?;
    let name_format = u16be(name, 0)?;
    if name_format > 1 {
        return Err("Unsupported font name-table format.".into());
    }
    let count = u16be(name, 2)? as usize;
    let strings = u16be(name, 4)? as usize;
    let records_end = 6 + 12 * count;
    if count > 4096 || records_end > name.len() || strings < records_end || strings > name.len() {
        return Err("Invalid font name table bounds.".into());
    }
    if name_format == 1 {
        let langs = u16be(name, records_end)? as usize;
        if records_end + 2 + 4 * langs > strings {
            return Err("Invalid font language-tag directory.".into());
        }
        for i in 0..langs {
            let at = records_end + 2 + 4 * i;
            let len = u16be(name, at)? as usize;
            let offset = u16be(name, at + 2)? as usize;
            if strings + offset + len > name.len() || len % 2 != 0 {
                return Err("Invalid font language-tag bounds.".into());
            }
        }
    }
    let mut names = Vec::new();
    for i in 0..count {
        let at = 6 + 12 * i;
        let platform = u16be(name, at)?;
        let encoding = u16be(name, at + 2)?;
        let lang = u16be(name, at + 4)?;
        let id = u16be(name, at + 6)?;
        let len = u16be(name, at + 8)? as usize;
        let offset = u16be(name, at + 10)? as usize;
        let raw = name
            .get(strings + offset..strings + offset + len)
            .ok_or("Font name string exceeds its table.")?;
        if ![1, 16].contains(&id)
            || !(platform == 0 || platform == 3 && [0, 1, 10].contains(&encoding))
        {
            continue;
        }
        if len % 2 != 0 || len / 2 > 200 {
            return Err("Invalid bounded Unicode font family name.".into());
        }
        let units = raw
            .chunks_exact(2)
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .collect::<Vec<_>>();
        let family = String::from_utf16(&units).map_err(|_| "Font family has invalid UTF-16.")?;
        if family.trim().is_empty() || family.chars().any(char::is_control) {
            continue;
        }
        names.push((
            if id == 16 { 10 } else { 0 }
                + if lang == 0x409 { 2 } else { 0 }
                + if platform == 3 { 1 } else { 0 },
            family,
        ));
    }
    names.sort_by(|a, b| b.0.cmp(&a.0));
    let family = names
        .first()
        .ok_or("No supported Unicode font family name was found.")?
        .1
        .clone();
    Ok(FontInfo { format, family })
}
pub(super) fn validate_license(license: &Value) -> Result<()> {
    fields(license, &["name", "source", "permissionConfirmed"], &[])?;
    string(license, "name", 5000, true)?;
    string(license, "source", 5000, true)?;
    if license["permissionConfirmed"] != true {
        return Err("Confirm permission to store and transfer this font with the project.".into());
    }
    Ok(())
}
pub(super) fn prepare_font(bytes: &[u8], license: &Value) -> Result<(ProjectAsset, Value)> {
    validate_license(license)?;
    let info = inspect_font(bytes)?;
    let id = uuid::Uuid::new_v4().to_string();
    let digest = checksum(bytes);
    let font = json!({"fontAssetId":id,"checksum":digest,"family":info.family,"format":info.format,"license":license});
    let asset = ProjectAsset {
        id: id.clone(),
        path: format!("assets/fonts/{id}.{}", info.format),
        mime_type: format!("font/{}", info.format),
        content: bytes.to_vec(),
        checksum: digest,
    };
    Ok((asset, font))
}
pub(super) fn verify_font<'a>(font: &Value, asset: &'a ProjectAsset) -> Result<&'a [u8]> {
    validate_license(&font["license"])?;
    let format = string(font, "format", 3, true)?;
    let id = string(font, "fontAssetId", 200, true)?;
    let digest = string(font, "checksum", 64, true)?;
    string(font, "family", 200, true)?;
    if !["ttf", "otf"].contains(&format)
        || !identity(id, 200)
        || !hex(digest)
        || asset.id != id
        || asset.path != format!("assets/fonts/{id}.{format}")
        || asset.mime_type != format!("font/{format}")
        || asset.checksum != digest
    {
        return Err("Project font asset identity, path or descriptor differs.".into());
    }
    let bytes = asset.content.as_slice();
    if bytes.len() > FONT_LIMIT || checksum(bytes) != digest {
        return Err("Project font checksum or size failed. Original writing is preserved.".into());
    }
    let info = inspect_font(bytes)?;
    if info.format != format || info.family != font["family"].as_str().unwrap() {
        return Err("Project font family/format differs from its definition.".into());
    }
    Ok(bytes)
}
/// Stage ONLY verified asset bytes into an already-owned private editor session.
/// Registration remains in crates/editor/src/fonts.rs in the disposable process.
pub(super) fn stage_private_font(
    directory: &Path,
    font: &Value,
    asset: &ProjectAsset,
) -> Result<Value> {
    let bytes = verify_font(font, asset)?;
    private_directory(directory)?;
    let root = directory.canonicalize().map_err(|e| e.to_string())?;
    let fonts = root.join("fonts");
    match fs::symlink_metadata(&fonts) {
        Ok(_) => private_directory(&fonts)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&fonts).map_err(|e| e.to_string())?;
        }
        Err(e) => return Err(e.to_string()),
    }
    let path = fonts.join(format!(
        "{}.{}",
        font["checksum"].as_str().unwrap(),
        font["format"].as_str().unwrap()
    ));
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err("Private staged font is not a regular file.".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err("Private staged font permissions are too broad.".into());
            }
        }
        if read_selected(&path, FONT_LIMIT)?.as_slice() != bytes {
            return Err("Private staged font has different bytes.".into());
        }
    } else {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|e| e.to_string())?;
        if let Err(e) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(e.to_string());
        }
    }
    Ok(json!({"path":path,"checksum":font["checksum"],"family":font["family"]}))
}
fn private_directory(directory: &Path) -> Result<()> {
    let m = fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("An owned private editor directory is required.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if m.permissions().mode() & 0o077 != 0 {
            return Err("The editor directory must be private.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "local_resource_tests.rs"]
pub(crate) mod tests;
