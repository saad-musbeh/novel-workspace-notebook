use super::*;
fn pack() -> Value {
    json!({"schemaVersion":1,"id":"synthetic","name":"Fixture glossary","language":"en","variant":"test","version":"1.0.0","source":"Synthetic author","license":"CC0-1.0","attribution":"Synthetic author","operations":["dictionary","synonyms"],"entries":[{"term":"bank","senses":[{"id":"finance","definition":"A place for money.","partOfSpeech":"noun","synonyms":["depository"],"related":["deposit"],"translations":[{"language":"ar","text":"مَصْرِف"}]},{"id":"river","definition":"The land beside a river.","partOfSpeech":"noun","synonyms":["shore"],"related":["river"],"translations":[]}],"notes":"Distinct senses."}],"checksum":"e43b256ec9fd243a39ae072c5a1f4cc1896aa23391662b30b900a1b00749cc0a"})
}
fn signed(mut value: Value) -> Value {
    value["checksum"] = json!(checksum(&value).unwrap());
    value
}
#[test]
fn retained_pack_checksum_and_distinct_senses_keep_declared_provenance() {
    // The fixed digest was generated independently from sorted UTF-8 JSON.
    let value = pack();
    assert_eq!(
        checksum(&value).unwrap(),
        value["checksum"].as_str().unwrap()
    );
    let source = parse_pack(&value).unwrap();
    assert_eq!(source.name, "Fixture glossary");
    assert_eq!(source.version, "1.0.0");
    assert_eq!(source.license, "CC0-1.0");
    assert_eq!(source.source, "Synthetic author");
    assert_eq!(source.attribution, "Synthetic author");
    assert_eq!(source.entries[0].senses.len(), 2);
    assert_eq!(source.entries[0].senses[0].synonyms, ["depository"]);
    assert_eq!(source.entries[0].senses[1].synonyms, ["shore"]);
    assert_eq!(source.entries[0].senses[0].related, ["deposit"]);
    assert_eq!(
        source.entries[0].senses[0].translations,
        [("ar".into(), "مَصْرِف".into())]
    );
    assert_eq!(source.entries[0].notes, "Distinct senses.");
    assert_eq!(lookup(&[source], "bank", "en", false), [(0, 0)]);
}
#[test]
fn multilingual_lookup_is_exact_and_case_changes_only_the_query() {
    let mut english = parse_pack(&pack()).unwrap();
    english.entries.push(Entry {
        term: "é".into(),
        senses: english.entries[0].senses.clone(),
        notes: String::new(),
    });
    let mut arabic = english.clone();
    arabic.language = "ar".into();
    arabic.entries[0].term = "مَرْحَبًا".into();
    let mut chinese = english.clone();
    chinese.language = "zh-Hans".into();
    chinese.entries[0].term = "世界 𠀀 👩🏽‍🚀".into();
    let sources = vec![english, arabic, chinese];
    let before = sources.clone();
    assert_eq!(lookup(&sources, "BANK", "en", false), []);
    assert_eq!(lookup(&sources, "BANK", "en", true), [(0, 0)]);
    assert_eq!(lookup(&sources, "مَرْحَبًا", "ar", false), [(1, 0)]);
    assert_eq!(lookup(&sources, "مرحبا", "ar", true), []);
    assert_eq!(lookup(&sources, "世界 𠀀 👩🏽‍🚀", "zh-Hans", false), [(2, 0)]);
    assert_eq!(lookup(&sources, "世界 𠀀 👩🏽‍🚀", "en", false), []);
    assert_eq!(lookup(&sources, "é", "en", false), [(0, 1)]);
    assert_eq!(lookup(&sources, "e\u{301}", "en", false), []);
    assert_eq!(sources, before);
}
#[test]
fn corrupt_and_malformed_packs_never_supply_lexical_results() {
    let mut broken = pack();
    broken["entries"][0]["senses"][0]["synonyms"][0] = json!("silently changed");
    assert!(parse_pack(&broken).unwrap_err().contains("checksum"));
    let mut examples = Vec::new();
    let mut v = pack();
    v["license"] = json!(" ");
    examples.push(v);
    let mut v = pack();
    v["source"] = Value::Null;
    examples.push(v);
    let mut v = pack();
    v["operations"] = json!(["online"]);
    examples.push(v);
    let mut v = pack();
    v["operations"] = json!(["dictionary"]);
    examples.push(v);
    let mut v = pack();
    v["entries"][0]["senses"][1]["id"] = json!("finance");
    examples.push(v);
    let mut v = pack();
    let duplicate = v["entries"][0].clone();
    v["entries"].as_array_mut().unwrap().push(duplicate);
    examples.push(v);
    let mut v = pack();
    v["entries"][0]["senses"] = json!([]);
    examples.push(v);
    let mut v = pack();
    v["entries"][0]["senses"][0]["translations"][0]["text"] = json!(" ");
    examples.push(v);
    let mut v = pack();
    v["entries"][0]["notes"] = json!("𠀀".repeat(1001));
    examples.push(v);
    let mut v = pack();
    v["entries"][0]["term"] = json!("𠀀".repeat(101));
    examples.push(v);
    for v in examples {
        assert!(parse_pack(&signed(v)).is_err());
    }
    let mut uppercase = pack();
    uppercase["checksum"] = json!(uppercase["checksum"].as_str().unwrap().to_uppercase());
    assert!(parse_pack(&uppercase).is_err());
    let mut oversized = pack();
    oversized["padding"] = json!("x".repeat(2_000_000));
    assert!(parse_pack(&signed(oversized)).unwrap_err().contains("2 MB"));
}
#[test]
fn source_documents_are_bounded_and_duplicate_pack_identities_are_refused() {
    let value = json!({"schemaVersion":1,"packs":[pack()]});
    assert_eq!(pack_sources(&value).unwrap().len(), 1);
    assert!(pack_sources(&json!({"schemaVersion":2,"packs":[]})).is_err());
    assert!(pack_sources(&json!({"schemaVersion":1,"packs":[pack(),pack()]})).is_err());
    assert!(pack_sources(&json!({"schemaVersion":1,"packs":vec![pack();21]})).is_err());
    assert!(pack_sources(&json!({"schemaVersion":1,"packs":null})).is_err());
}
#[test]
fn alphabet_glossary_versions_are_named_read_only_project_sources() {
    let value = json!({"schemaVersion":1,"systems":[{"schemaVersion":1,"id":"fictional","name":"Fictional alphabet","mappingVersion":1,"glossary":[{"symbol":"\u{e000}𠀀","meaning":"A kept promise مِ 中文"}]},{"schemaVersion":1,"id":"fictional","name":"Fictional alphabet","mappingVersion":2,"glossary":[{"symbol":"\u{e000}𠀀","meaning":"A revised promise"}]}]});
    let sources = alphabet_sources(&value).unwrap();
    assert_eq!(sources.len(), 2);
    assert_eq!(sources[0].version, "mapping 1");
    assert_eq!(sources[1].version, "mapping 2");
    assert_eq!(sources[0].source, "Saved project alphabet glossary");
    assert!(sources[0]
        .license
        .contains("no external dictionary license"));
    assert_eq!(
        lookup(&sources, "\u{e000}𠀀", "any", false),
        [(0, 0), (1, 0)]
    );
    assert_eq!(lookup(&sources, "\u{e000}𠀀", "en", false), []);
    let mut invalid = value.clone();
    invalid["systems"][0]["glossary"][0]["meaning"] = Value::Null;
    assert!(alphabet_sources(&invalid).is_err());
    assert_eq!(alphabet_sources(&value).unwrap(), sources);
    let mut blank = value.clone();
    blank["systems"][0]["glossary"][0]["meaning"] = json!("");
    assert_eq!(
        alphabet_sources(&blank).unwrap()[0].entries[0].senses[0].definition,
        ""
    );
}
#[test]
fn canonical_keys_follow_retained_utf16_order_and_keep_exact_strings() {
    assert_eq!(
        canonical(&json!({"\u{e000}":1,"𐀀":2,"a":"مِ 中文 𠀀 👩🏽‍🚀\n"})),
        "{\"a\":\"مِ 中文 𠀀 👩🏽‍🚀\\n\",\"𐀀\":2,\"\u{e000}\":1}"
    );
    assert_eq!(
        checksum(&pack()).unwrap(),
        checksum(
            &serde_json::from_str::<Value>(&serde_json::to_string_pretty(&pack()).unwrap())
                .unwrap()
        )
        .unwrap()
    );
}
