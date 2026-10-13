//! Project-scoped immutable lexical identities across all retained history.
//! Checks execute before mutations in the caller's existing transaction.
use crate::workspace::{err, hash, validate_current_source, validate_data};
use serde_json::Value;
use sqlx::{Sqlite, Transaction};
use std::collections::{HashMap, HashSet};

fn packs(doc: &Value) -> Result<Vec<(&str, &str, &Value)>, String> {
    if doc["schemaVersion"].as_u64() != Some(1) {
        return Err("Unsupported retained pack document version.".into());
    }
    let list = doc["packs"]
        .as_array()
        .filter(|p| p.len() <= 20)
        .ok_or("Invalid retained pack collection.")?;
    let mut ids = HashSet::new();
    let mut versions = Vec::new();
    for pack in list {
        let id = pack["id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 100)
            .ok_or("Invalid retained pack identity.")?;
        if !ids.insert(id) {
            return Err("Duplicate retained active pack identity.".into());
        }
        // Older storage accepted packs without a usable version. Keep those
        // opaque records compatible; never infer an immutable version for them.
        if let Some(version) = pack["version"].as_str().filter(|s| !s.trim().is_empty()) {
            versions.push((id, version, pack));
        }
    }
    Ok(versions)
}
fn canonical(value: &Value, out: &mut Vec<u8>) -> Result<(), String> {
    match value {
        Value::Object(map) => {
            out.push(b'{');
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort();
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(&mut *out, key).map_err(|e| e.to_string())?;
                out.push(b':');
                canonical(&map[key], out)?;
            }
            out.push(b'}');
        }
        Value::Array(values) => {
            out.push(b'[');
            for (i, value) in values.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                canonical(value, out)?;
            }
            out.push(b']');
        }
        _ => serde_json::to_writer(out, value).map_err(|e| e.to_string())?,
    }
    Ok(())
}
/// Stores only identity/version and derived full-content hashes, not every body.
#[derive(Default)]
pub(crate) struct VersionIndex {
    fingerprints: HashMap<(String, String), String>,
    has_history: bool,
}
impl VersionIndex {
    pub(crate) fn observe(&mut self, doc: &Value) -> Result<(), String> {
        self.has_history = true;
        for (id, version, pack) in packs(doc)? {
            let mut bytes = Vec::new();
            canonical(pack, &mut bytes)?;
            let fingerprint = hash(&bytes);
            let key = (id.to_owned(), version.to_owned());
            if self
                .fingerprints
                .get(&key)
                .is_some_and(|old| old != &fingerprint)
            {
                return Err("A retained language-pack identity/version is immutable. Use a new pack version.".into());
            }
            self.fingerprints.insert(key, fingerprint);
        }
        Ok(())
    }
}
pub fn validate_retained_pack_versions(previous: &Value, candidate: &Value) -> Result<(), String> {
    let mut versions = VersionIndex::default();
    versions.observe(previous)?;
    versions.observe(candidate)
}

/// One bounded payload at a time, without silently truncating history. The same
/// transaction checks checksums/ownership/schema before comparing the candidate.
pub(crate) async fn checked_history(
    tx: &mut Transaction<'_, Sqlite>,
    pid: &str,
) -> Result<VersionIndex, String> {
    let mut versions = VersionIndex::default();
    let mut after: Option<i64> = None;
    loop {
        let row:Option<(i64,Option<String>,String,i64,String)>=sqlx::query_as("SELECT rowid,CASE WHEN length(CAST(payload AS BLOB))<=8000000 THEN payload END,checksum,schema_version,operation FROM revision_envelope WHERE project_id=? AND entity_id=? AND entity_type='projectData' AND (? IS NULL OR rowid>?) ORDER BY rowid LIMIT 1")
            .bind(pid).bind(format!("{pid}:languagePacks")).bind(after).bind(after).fetch_optional(&mut **tx).await.map_err(err)?;
        let Some((rowid, payload, checksum, schema, operation)) = row else {
            break;
        };
        after = Some(rowid);
        let payload = payload.ok_or("Retained language-pack payload exceeds its size limit.")?;
        if schema != 1 || operation != "languagePacks" || hash(payload.as_bytes()) != checksum {
            return Err("Retained language-pack schema, operation or checksum failed. History was preserved.".into());
        }
        let doc: Value = serde_json::from_str(&payload)
            .map_err(|_| "Invalid retained language-pack payload.")?;
        validate_data("languagePacks", &doc)?;
        versions.observe(&doc)?;
    }
    Ok(versions)
}
/// Ordinary saves cannot implicitly acknowledge competing or unprojected heads.
pub(crate) async fn validate_authoring(
    tx: &mut Transaction<'_, Sqlite>,
    pid: &str,
    current: Option<&str>,
    candidate: &Value,
) -> Result<(), String> {
    let mut history = checked_history(tx, pid).await?;
    if let Some(head) = current {
        validate_current_source(
            tx,
            pid,
            &format!("{pid}:languagePacks"),
            "projectData",
            head,
        )
        .await?;
    } else if history.has_history {
        return Err("Conflict: resolve retained language-pack heads before saving.".into());
    }
    history.observe(candidate)
}
pub(crate) async fn validate_candidate(
    tx: &mut Transaction<'_, Sqlite>,
    pid: &str,
    candidate: &Value,
) -> Result<(), String> {
    checked_history(tx, pid).await?.observe(candidate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn older_version_reuse_with_changed_payload_is_refused() {
        let old = json!({"schemaVersion":1,"packs":[{"id":"test","version":"1","entries":["مِ 𠀀 e\u{301}"]}]});
        let mut changed = old.clone();
        changed["packs"][0]["entries"][0] = json!("other");
        assert!(validate_retained_pack_versions(&old, &changed).is_err());
        validate_retained_pack_versions(&old, &old).unwrap();
        changed["packs"][0]["version"] = json!("2");
        validate_retained_pack_versions(&old, &changed).unwrap();
    }
    #[test]
    fn exact_old_value_and_unversioned_legacy_records_remain_compatible() {
        let old =
            json!({"schemaVersion":1,"packs":[{"id":"test","version":"1","entries":["中文"]}]});
        let newer =
            json!({"schemaVersion":1,"packs":[{"id":"test","version":"2","entries":["new"]}]});
        validate_retained_pack_versions(&newer, &old).unwrap();
        validate_retained_pack_versions(&old, &json!({"schemaVersion":1,"packs":[]})).unwrap();
        let legacy = json!({"schemaVersion":1,"packs":[{"id":"legacy","entries":["مِ"]}]});
        validate_retained_pack_versions(&legacy, &legacy).unwrap();
    }
    #[test]
    fn malformed_collection_identity_is_refused() {
        for bad in [
            json!({"schemaVersion":2,"packs":[]}),
            json!({"schemaVersion":1,"packs":[{"id":"test","version":"1"},{"id":"test","version":"2"}]}),
        ] {
            assert!(
                validate_retained_pack_versions(&bad, &json!({"schemaVersion":1,"packs":[]}))
                    .is_err()
            );
        }
    }
}
