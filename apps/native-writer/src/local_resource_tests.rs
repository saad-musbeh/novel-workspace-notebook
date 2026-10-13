use super::*;
use std::collections::BTreeMap;

pub(crate) fn pack_fixture() -> Value {
    let mut p = json!({"schemaVersion":1,"id":"synthetic.lexicon","name":"Synthetic multilingual test pack","language":"ar","variant":"test","version":"1","source":"Synthetic fixture; no external data","license":"Synthetic fixture permission only","attribution":"Created for tests","operations":["dictionary","synonyms","glossary"],"entries":[{"term":"مِ 中文 e\u{301} 𠀀","senses":[{"id":"sense-1","definition":"Exact test sense","partOfSpeech":"noun","synonyms":["لفظ"],"related":["相关"],"translations":[{"language":"en","text":"test"}]}]}],"checksum":""});
    p["checksum"] = json!(pack_checksum(&p).unwrap());
    p
}
fn put16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_be_bytes());
}
fn put32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_be_bytes());
}
fn cff_fixture() -> Vec<u8> {
    let name = b"Synthetic";
    let mut cff = vec![1, 0, 4, 4, 0, 1, 1, 1, (name.len() + 1) as u8];
    cff.extend(name);
    let charstrings_offset = cff.len() + 11 + 2 + 2;
    cff.extend([0, 1, 1, 1, 7, 29]);
    cff.extend((charstrings_offset as u32).to_be_bytes());
    cff.push(17);
    cff.extend([0, 0, 0, 0]);
    cff.extend([0, 1, 1, 1, 2, 14]);
    cff
}
/// One empty .notdef glyph, a sentinel-only format-4 cmap, Unicode names and
/// genuine SFNT directory/table/whole-font checksums. No external font bytes.
pub(crate) fn synthetic_sfnt(otf: bool) -> Vec<u8> {
    let mut tables = BTreeMap::<[u8; 4], Vec<u8>>::new();
    let mut head = vec![0; 54];
    put32(&mut head, 0, 0x10000);
    put32(&mut head, 4, 0x10000);
    put32(&mut head, 12, 0x5f0f3cf5);
    put16(&mut head, 18, 1000);
    put16(&mut head, 46, 8);
    tables.insert(*b"head", head);
    let mut hhea = vec![0; 36];
    put32(&mut hhea, 0, 0x10000);
    put16(&mut hhea, 4, 800);
    put16(&mut hhea, 6, (-200i16) as u16);
    put16(&mut hhea, 10, 500);
    put16(&mut hhea, 18, 1);
    put16(&mut hhea, 34, 1);
    tables.insert(*b"hhea", hhea);
    let mut hmtx = vec![0; 4];
    put16(&mut hmtx, 0, 500);
    tables.insert(*b"hmtx", hmtx);
    let mut maxp = vec![0; if otf { 6 } else { 32 }];
    put32(&mut maxp, 0, if otf { 0x5000 } else { 0x10000 });
    put16(&mut maxp, 4, 1);
    if !otf {
        put16(&mut maxp, 14, 1);
    }
    tables.insert(*b"maxp", maxp);
    let mut cmap = vec![0; 36];
    put16(&mut cmap, 2, 1);
    put16(&mut cmap, 4, 3);
    put16(&mut cmap, 6, 1);
    put32(&mut cmap, 8, 12);
    put16(&mut cmap, 12, 4);
    put16(&mut cmap, 14, 24);
    put16(&mut cmap, 18, 2);
    put16(&mut cmap, 20, 2);
    put16(&mut cmap, 26, 0xffff);
    put16(&mut cmap, 30, 0xffff);
    put16(&mut cmap, 32, 1);
    tables.insert(*b"cmap", cmap);
    let family = "Synthetic Test Family";
    let units = family.encode_utf16().collect::<Vec<_>>();
    let mut name = vec![0; 18 + 2 * units.len()];
    put16(&mut name, 2, 1);
    put16(&mut name, 4, 18);
    put16(&mut name, 6, 3);
    put16(&mut name, 8, 1);
    put16(&mut name, 10, 0x409);
    put16(&mut name, 12, 1);
    put16(&mut name, 14, (2 * units.len()) as u16);
    for (i, v) in units.into_iter().enumerate() {
        put16(&mut name, 18 + 2 * i, v);
    }
    tables.insert(*b"name", name);
    let mut post = vec![0; 32];
    put32(&mut post, 0, 0x30000);
    tables.insert(*b"post", post);
    let mut os2 = vec![0; 96];
    put16(&mut os2, 0, 4);
    put16(&mut os2, 2, 500);
    put16(&mut os2, 4, 400);
    put16(&mut os2, 6, 5);
    put16(&mut os2, 68, 800);
    put16(&mut os2, 70, (-200i16) as u16);
    put16(&mut os2, 74, 800);
    put16(&mut os2, 76, 200);
    tables.insert(*b"OS/2", os2);
    if otf {
        tables.insert(*b"CFF ", cff_fixture());
    } else {
        tables.insert(*b"glyf", vec![]);
        tables.insert(*b"loca", vec![0; 4]);
    }
    let count = tables.len();
    let mut bytes = vec![0; 12 + 16 * count];
    bytes[..4].copy_from_slice(if otf { b"OTTO" } else { b"\0\x01\0\0" });
    put16(&mut bytes, 4, count as u16);
    let power = 1usize << (usize::BITS - 1 - count.leading_zeros());
    put16(&mut bytes, 6, (16 * power) as u16);
    put16(&mut bytes, 8, power.trailing_zeros() as u16);
    put16(&mut bytes, 10, (16 * (count - power)) as u16);
    for (i, (tag, content)) in tables.into_iter().enumerate() {
        let at = 12 + 16 * i;
        let offset = bytes.len();
        bytes[at..at + 4].copy_from_slice(&tag);
        put32(&mut bytes, at + 4, table_sum(&content, tag == *b"head"));
        put32(&mut bytes, at + 8, offset as u32);
        put32(&mut bytes, at + 12, content.len() as u32);
        bytes.extend(content);
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
    }
    repair_font(&mut bytes);
    bytes
}
pub(crate) fn table_offset(bytes: &[u8], tag: &[u8; 4]) -> usize {
    for i in 0..u16be(bytes, 4).unwrap() as usize {
        let at = 12 + 16 * i;
        if &bytes[at..at + 4] == tag {
            return u32be(bytes, at + 8).unwrap() as usize;
        }
    }
    panic!("missing test table");
}
pub(crate) fn repair_font(bytes: &mut [u8]) {
    let head = table_offset(bytes, b"head");
    put32(bytes, head + 8, 0);
    for i in 0..u16be(bytes, 4).unwrap() as usize {
        let at = 12 + 16 * i;
        let offset = u32be(bytes, at + 8).unwrap() as usize;
        let len = u32be(bytes, at + 12).unwrap() as usize;
        let sum = table_sum(&bytes[offset..offset + len], &bytes[at..at + 4] == b"head");
        put32(bytes, at + 4, sum);
    }
    let adjust = 0xb1b0afbau32.wrapping_sub(table_sum(bytes, false));
    put32(bytes, head + 8, adjust);
}
pub(crate) fn license_fixture() -> Value {
    json!({"name":"Synthetic test license","source":"Test generator only","permissionConfirmed":true})
}
#[test]
fn checksum_and_canonical_json_match_standard_utf8() {
    assert_eq!(
        checksum(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        canonical_json(&json!({"z":"مِ","a":["e\u{301}","𠀀"]})).unwrap(),
        "{\"a\":[\"é\",\"𠀀\"],\"z\":\"مِ\"}"
    );
    assert_eq!(
        canonical_json(&json!({"\u{e000}":1,"𠀀":2})).unwrap(),
        "{\"𠀀\":2,\"\":1}"
    );
    assert!(canonical_json(&json!(1.5)).is_err());
}
#[test]
fn pack_import_preserves_exact_multilingual_senses_and_checks_hash() {
    let p = pack_fixture();
    // Golden independently computed with the retained frontend canonicalJson.
    assert_eq!(
        p["checksum"],
        "4701381df43708e5e9e08c6caedade743679668539ca7599a89fdc4da9d0d5cc"
    );
    let bytes = serde_json::to_vec(&p).unwrap();
    assert_eq!(import_pack(&bytes).unwrap(), p);
    let mut bad = p;
    bad["entries"][0]["term"] = json!("different");
    assert!(import_pack(&serde_json::to_vec(&bad).unwrap())
        .unwrap_err()
        .contains("checksum"));
}
#[test]
fn pack_schema_refuses_unknown_fields_duplicates_and_undeclared_operations() {
    for mutation in 0..6 {
        let mut p = pack_fixture();
        match mutation {
            0 => p["extra"] = json!("unknown"),
            1 => p["source"] = json!(""),
            2 => p["operations"] = json!(["dictionary"]),
            3 => {
                let duplicate = p["entries"][0].clone();
                p["entries"].as_array_mut().unwrap().push(duplicate);
            }
            4 => p["entries"][0]["senses"][0]["translations"][0]["text"] = json!("x".repeat(201)),
            _ => p["entries"][0]["senses"][0]["partOfSpeech"] = json!(""),
        };
        p["checksum"] = json!(pack_checksum(&p).unwrap());
        assert!(validate_pack(&p).is_err(), "mutation {mutation}");
    }
    assert!(import_pack(&vec![b' '; PACK_LIMIT + 1]).is_err());
    assert!(import_pack(b"{invalid}").is_err());
}
#[test]
fn historical_pack_identity_version_cannot_be_repurposed() {
    let first = pack_fixture();
    let mut versions = PackVersions::default();
    versions
        .observe(&json!({"schemaVersion":1,"packs":[first.clone()]}))
        .unwrap();
    let mut changed = first.clone();
    changed["entries"][0]["notes"] = json!("new data");
    changed["checksum"] = json!(pack_checksum(&changed).unwrap());
    assert!(versions
        .install(&json!({"schemaVersion":1,"packs":[]}), &changed)
        .is_err());
    changed["version"] = json!("2");
    changed["checksum"] = json!(pack_checksum(&changed).unwrap());
    let doc = versions
        .install(
            &json!({"schemaVersion":1,"packs":[first.clone()]}),
            &changed,
        )
        .unwrap();
    assert_eq!(doc["packs"], json!([changed]));
    assert_eq!(
        versions
            .install(&json!({"schemaVersion":1,"packs":[]}), &first)
            .unwrap()["packs"],
        json!([first])
    );
}
#[test]
fn synthetic_true_type_and_cff_sfnt_have_valid_names_and_checksums() {
    for otf in [false, true] {
        let bytes = synthetic_sfnt(otf);
        let info = inspect_font(&bytes).unwrap();
        assert_eq!(info.family, "Synthetic Test Family");
        assert_eq!(info.format, if otf { "otf" } else { "ttf" });
        assert_eq!(table_sum(&bytes, false), 0xb1b0afba);
    }
}
#[test]
fn font_license_requires_source_and_explicit_permission() {
    let bytes = synthetic_sfnt(false);
    for field in ["name", "source", "permissionConfirmed"] {
        let mut license = license_fixture();
        license[field] = if field == "permissionConfirmed" {
            json!(false)
        } else {
            json!("")
        };
        assert!(prepare_font(&bytes, &license).is_err());
    }
    let (asset, font) = prepare_font(&bytes, &license_fixture()).unwrap();
    assert_eq!(verify_font(&font, &asset).unwrap(), bytes);
}
#[test]
fn font_tables_reject_bounds_corruption_names_and_embedding_restriction() {
    let original = synthetic_sfnt(false);
    for mutation in 0..7 {
        let mut b = original.clone();
        match mutation {
            0 => b[..4].copy_from_slice(b"ttcf"),
            1 => put16(&mut b, 4, 257),
            2 => put32(&mut b, 12 + 8, u32::MAX),
            3 => {
                let at = table_offset(&b, b"head");
                b[at + 18] ^= 1;
            }
            4 => {
                let at = table_offset(&b, b"name");
                put16(&mut b, at + 4, 0);
                repair_font(&mut b);
            }
            5 => {
                let at = table_offset(&b, b"name");
                put16(&mut b, at + 18, 0xd800);
                repair_font(&mut b);
            }
            _ => {
                let at = table_offset(&b, b"OS/2");
                put16(&mut b, at + 8, 2);
                repair_font(&mut b);
            }
        }
        assert!(inspect_font(&b).is_err(), "mutation {mutation}");
    }
    assert!(inspect_font(&vec![0; FONT_LIMIT + 1]).is_err());
}
#[test]
fn asset_verification_refuses_family_path_content_or_checksum_mismatch() {
    let (asset, font) = prepare_font(&synthetic_sfnt(false), &license_fixture()).unwrap();
    for mutation in 0..4 {
        let mut a = asset.clone();
        let mut f = font.clone();
        match mutation {
            0 => a.path = "../outside.ttf".into(),
            1 => a.content[0] ^= 1,
            2 => f["family"] = json!("Wrong family"),
            _ => a.checksum = "0".repeat(64),
        };
        assert!(verify_font(&f, &a).is_err());
    }
}
#[test]
fn bounded_selected_reads_touch_only_temporary_fixtures() {
    let temp = tempfile::tempdir().unwrap();
    let p = temp.path().join("selected.json");
    fs::write(&p, b"1234").unwrap();
    assert_eq!(read_selected(&p, 4).unwrap(), b"1234");
    assert!(read_selected(&p, 3).is_err());
    assert!(read_selected(temp.path(), 4).is_err());
}
#[test]
fn private_staging_is_verified_idempotent_and_rejects_symlink_escape() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("rust-editor-synthetic");
    fs::create_dir(&dir).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let (asset, font) = prepare_font(&synthetic_sfnt(false), &license_fixture()).unwrap();
    let staged = stage_private_font(&dir, &font, &asset).unwrap();
    assert_eq!(stage_private_font(&dir, &font, &asset).unwrap(), staged);
    let path = Path::new(staged["path"].as_str().unwrap());
    assert!(path.starts_with(dir.join("fonts")));
    assert_eq!(fs::read(path).unwrap(), verify_font(&font, &asset).unwrap());
    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(dir.join("fonts"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        let other = temp.path().join("rust-editor-other");
        fs::create_dir(&other).unwrap();
        fs::set_permissions(&other, fs::Permissions::from_mode(0o700)).unwrap();
        symlink(dir.join("fonts"), other.join("fonts")).unwrap();
        assert!(stage_private_font(&other, &font, &asset).is_err());
        fs::remove_file(path).unwrap();
        symlink(temp.path().join("outside.ttf"), path).unwrap();
        assert!(stage_private_font(&dir, &font, &asset).is_err());
        assert!(!temp.path().join("outside.ttf").exists());
    }
}

#[test]
fn bounded_parser_refuses_cff_indices_and_cmap_offsets_without_panicking() {
    let mut otf = synthetic_sfnt(true);
    let cff = table_offset(&otf, b"CFF ");
    otf[cff + 6] = 0;
    repair_font(&mut otf);
    assert!(inspect_font(&otf).unwrap_err().contains("CFF"));
    let mut ttf = synthetic_sfnt(false);
    let cmap = table_offset(&ttf, b"cmap");
    put16(&mut ttf, cmap + 12 + 22, 0xfffe);
    repair_font(&mut ttf);
    assert!(inspect_font(&ttf).unwrap_err().contains("cmap"));
    for otf in [false, true] {
        let bytes = synthetic_sfnt(otf);
        for len in 0..bytes.len() {
            let _ = inspect_font(&bytes[..len]);
        }
    }
}

#[test]
fn an_invalid_font_never_creates_a_staging_directory() {
    let temp = tempfile::tempdir().unwrap();
    let (asset, mut font) = prepare_font(&synthetic_sfnt(false), &license_fixture()).unwrap();
    font["family"] = json!("Wrong family");
    assert!(stage_private_font(temp.path(), &font, &asset).is_err());
    assert!(!temp.path().join("fonts").exists());
}
