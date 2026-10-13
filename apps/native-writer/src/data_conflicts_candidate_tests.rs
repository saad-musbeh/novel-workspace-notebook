use super::*;
fn block(text: &str) -> Value {
    json!({"type":"paragraph","attrs":{"lang":"ar","dir":"auto"},"content":[{"type":"text","text":text,"marks":[{"type":"bold"}]}]})
}
fn alternatives() -> Value {
    let original = block("Original مِ 中文 𠀀 👩🏽‍🚀");
    json!({"schemaVersion":1,"scenes":[{"sceneId":"scene","groups":[{"id":"group","label":"Paragraph","sceneRevisionId":"source-revision","anchor":{"blockIndex":0,"documentHash":"a".repeat(64),"block":original},"originalId":"original","activeId":"original","versions":[{"id":"original","label":"Original","block":original}]}]}]})
}
fn added_version(mut value: Value, id: &str, text: &str) -> Value {
    value["scenes"][0]["groups"][0]["versions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":id,"label":id,"block":block(text)}));
    value["scenes"][0]["groups"][0]["activeId"] = json!(id);
    value
}
#[test]
fn requested_paragraph_candidate_retains_original_and_both_added_versions_exactly() {
    let original = alternatives();
    let left = added_version(original.clone(), "left", "Left مِ 中文");
    let right = added_version(original.clone(), "right", "Right 世界 👩🏽‍🚀");
    let candidate = combined_candidate(
        "paragraphAlternatives",
        &left,
        &[left.clone(), right.clone()],
    )
    .unwrap();
    let versions = candidate["scenes"][0]["groups"][0]["versions"]
        .as_array()
        .unwrap();
    assert_eq!(versions.len(), 3);
    assert_eq!(candidate["scenes"][0]["groups"][0]["activeId"], "left");
    for source in [&original, &left, &right] {
        for version in source["scenes"][0]["groups"][0]["versions"]
            .as_array()
            .unwrap()
        {
            assert!(versions.contains(version));
        }
    }
    assert_eq!(
        candidate["scenes"][0]["groups"][0]["anchor"],
        left["scenes"][0]["groups"][0]["anchor"]
    );
    assert_eq!(
        candidate["scenes"][0]["groups"][0]["sceneRevisionId"],
        left["scenes"][0]["groups"][0]["sceneRevisionId"]
    );
}
#[test]
fn requested_writing_system_candidate_keeps_every_mapping_version_and_base_assignments() {
    let a = json!({"schemaVersion":1,"id":"alphabet","mappingVersion":1,"mappings":[{"input":"a","output":"\u{e000}"}]});
    let b = json!({"schemaVersion":1,"id":"alphabet","mappingVersion":2,"mappings":[{"input":"a","output":"\u{e001}"}]});
    let left = json!({"schemaVersion":1,"systems":[a],"assignments":[{"scene":{"kind":"scene","id":"scene"},"definitionId":"alphabet","mappingVersion":1}]});
    let right = json!({"schemaVersion":1,"systems":[a,b],"assignments":[]});
    let candidate =
        combined_candidate("writingSystems", &left, &[left.clone(), right.clone()]).unwrap();
    assert_eq!(candidate["systems"], right["systems"]);
    assert_eq!(candidate["assignments"], left["assignments"]);
    let mut corrupt = right.clone();
    corrupt["systems"][0]["mappings"][0]["output"] = json!("changed");
    assert!(combined_candidate("writingSystems", &left, &[left.clone(), corrupt]).is_err());
}
#[test]
fn translation_candidate_retains_sources_and_additions_while_author_base_chooses_edited_text() {
    let draft = |id: &str, text: &str| json!({"id":id,"sceneId":"scene","sceneRevisionId":"source","sourceLanguage":"ar","targetLanguage":"en","sourceText":"مَرْحَبًا 世界","text":text});
    let left = json!({"schemaVersion":1,"drafts":[draft("common","Chosen author edit"),draft("left","Left addition")]});
    let right = json!({"schemaVersion":1,"drafts":[draft("common","Other edit"),draft("right","Right addition")]});
    let candidate =
        combined_candidate("sceneTranslations", &left, &[left.clone(), right.clone()]).unwrap();
    assert_eq!(candidate["drafts"].as_array().unwrap().len(), 3);
    assert_eq!(candidate["drafts"][0], left["drafts"][0]);
    assert_eq!(candidate["drafts"][2], right["drafts"][1]);
    let mut corrupt = right.clone();
    corrupt["drafts"][0]["sourceText"] = json!("Changed immutable source");
    assert!(combined_candidate("sceneTranslations", &left, &[left.clone(), corrupt]).is_err());
    let historical = json!({"schemaVersion":1,"drafts":[]});
    assert!(combined_candidate("sceneTranslations", &historical, &[left, right]).is_err());
}
#[test]
fn malformed_conflicting_and_oversized_candidates_refuse_without_mutating_sources() {
    let original = alternatives();
    let left = added_version(original.clone(), "same", "Left");
    let right = added_version(original.clone(), "same", "Right");
    assert!(combined_candidate("paragraphAlternatives", &left, &[left.clone(), right]).is_err());
    assert_eq!(left, added_version(original.clone(), "same", "Left"));
    let mut malformed = original.clone();
    malformed["scenes"][0]["groups"][0]["versions"][0]["block"] = json!({"type":"unknown"});
    assert!(combined_candidate("paragraphAlternatives", &original, &[malformed]).is_err());
    let mut absent = original.clone();
    absent["scenes"][0]["groups"][0]["originalId"] = json!("missing");
    assert!(combined_candidate("paragraphAlternatives", &original, &[absent]).is_err());
    let mut crowded = original.clone();
    for i in 0..20 {
        crowded = added_version(crowded, &format!("extra-{i}"), "retained");
    }
    assert!(
        combined_candidate("paragraphAlternatives", &original, &[crowded])
            .unwrap_err()
            .contains("20-per-group")
    );
    assert!(combined_candidate("notebookTools", &json!({}), &[])
        .unwrap_err()
        .contains("Other categories"));
    assert!(combined_candidate("languagePacks", &json!({}), &[]).is_err());
    assert!(combined_candidate(
        "paragraphAlternatives",
        &original,
        &vec![original.clone(); 65]
    )
    .unwrap_err()
    .contains("64-parent"));
}
