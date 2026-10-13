use novel_storage::{Scene, Storage, SyncCursor};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
fn scene() -> Scene {
    Scene {
        schema_version: 1,
        title: "Synthetic scene مِ 中文".into(),
        document: json!({"type":"doc","content":[{"type":"paragraph","attrs":{"lang":"ar","dir":"auto"},"content":[{"type":"text","text":"مِ 中文 e\u{301} \u{e000} 𠀀 👩🏽‍🚀","marks":[{"type":"bold"}]}]}]}),
    }
}
async fn fixture() -> (tempfile::TempDir, Storage, String, String) {
    let d = tempfile::tempdir().unwrap();
    let s = Storage::open(&d.path().join("board.sqlite")).await.unwrap();
    let p = s.create_project("Synthetic board project").await.unwrap();
    let c = s.create_chapter(&p.id, "Chapter").await.unwrap();
    let x = s.create_scene(&c.id, scene()).await.unwrap();
    (d, s, p.id, x.id)
}
fn card(id: &str, reference: Value) -> Value {
    json!({"id":id,"kind":"scene","title":"نور 明","body":"مِ 中文 e\u{301} \u{e000} 𠀀 👩🏽‍🚀","reference":reference,"x":50,"y":-25.5,"groupId":"group","labels":["مِ","中文"]})
}
fn planning(sid: &str) -> Value {
    json!({"schemaVersion":1,"fields":[],"records":[{"id":"hero","kind":"character","name":"نور 明","aliases":[],"notes":"Synthetic author notes","links":[{"kind":"scene","id":sid}],"values":{"custom":[{"kind":"scene","id":sid}]}}],"facts":[{"id":"fact","subjectId":"hero","property":"Goal","value":"مِ 中文","status":"fact","confirmed":true,"observerId":null,"source":"Synthetic author","eventId":null,"untilEventId":null}],"plotPoints":[],"boards":[{"id":"board","name":"Synthetic board","cards":[card(sid,json!({"kind":"scene","id":sid})),{"id":"card-b","kind":"note","title":"Garden 中文","body":"Second","reference":null,"x":400,"y":220,"groupId":null,"labels":[]}],"groups":[{"id":"group","name":"Synthetic group"}],"edges":[{"id":"edge","fromId":sid,"toId":"card-b","kind":"custom","label":"يَكشف 中文 e\u{301} \u{e000} 👩🏽‍🚀","confirmed":false}]}],"lanes":[],"events":[{"id":"event","title":"Synthetic occurrence","notes":"","laneIds":[],"sceneIds":[sid],"time":{"kind":"unknown"}}],"constraints":[],"issues":[],"readerReveals":[{"id":"reveal","factId":"fact","eventId":null,"scene":{"kind":"scene","id":sid},"position":0,"notes":"Synthetic reveal"}]})
}
fn copied(original: &Value, old: &str, new: &str) -> Value {
    let mut out = original.clone();
    out["records"][0]["links"][0]["id"] = json!(new);
    out["records"][0]["values"]["custom"][0]["id"] = json!(new);
    out["boards"][0]["cards"][0]["reference"]["id"] = json!(new);
    out["events"][0]["sceneIds"] = json!([new]);
    out["readerReveals"][0]["scene"]["id"] = json!(new);
    assert_eq!(out["boards"][0]["cards"][0]["id"], old);
    assert_eq!(out["boards"][0]["edges"][0]["fromId"], old);
    out
}
#[tokio::test]
async fn full_scene_cards_keep_board_ids_edges_and_exact_unicode_across_archives_branches_and_sync()
{
    let (_d, s, pid, sid) = fixture().await;
    let payload = planning(&sid);
    let saved = s
        .save_project_data(&pid, "planning", payload.clone(), None)
        .await
        .unwrap();
    assert_eq!(
        s.load_project_data(&pid, "planning")
            .await
            .unwrap()
            .unwrap(),
        saved
    );
    let original = s.load_scene(&sid).await.unwrap();
    let archive = s.export_archive(&pid).await.unwrap();
    let imported = s.import_archive(&archive).await.unwrap();
    let imported_sid = &imported.scenes[0].id;
    let expected = copied(&payload, &sid, imported_sid);
    assert_eq!(
        s.load_project_data(&imported.project.id, "planning")
            .await
            .unwrap()
            .unwrap()
            .payload,
        expected
    );
    assert_eq!(imported.scenes[0].scene, original.scene);
    let reimported = s
        .import_archive(&s.export_archive(&imported.project.id).await.unwrap())
        .await
        .unwrap();
    assert_eq!(
        s.load_project_data(&reimported.project.id, "planning")
            .await
            .unwrap()
            .unwrap()
            .payload,
        copied(&payload, &sid, &reimported.scenes[0].id)
    );
    let branch = s
        .create_project_branch(&pid, "Synthetic branch")
        .await
        .unwrap();
    let branch_scene = s
        .open_project(&branch.project_id)
        .await
        .unwrap()
        .scenes
        .remove(0);
    assert_eq!(
        s.load_project_data(&branch.project_id, "planning")
            .await
            .unwrap()
            .unwrap()
            .payload,
        copied(&payload, &sid, &branch_scene.id)
    );
    assert_eq!(branch_scene.scene, original.scene);
    let peer_dir = tempfile::tempdir().unwrap();
    let peer = Storage::open(&peer_dir.path().join("replica.sqlite"))
        .await
        .unwrap();
    let source = s.sync_cursor().await.unwrap();
    let target = peer.sync_cursor().await.unwrap();
    let page = s
        .pull_revisions(
            &pid,
            SyncCursor {
                epoch: source.epoch.clone(),
                sequence: 0,
            },
            1000,
        )
        .await
        .unwrap();
    peer.ingest_revisions(
        &pid,
        &target.epoch,
        &source.epoch,
        s.sync_manifest(&pid).await.unwrap(),
        page.revisions,
    )
    .await
    .unwrap();
    assert_eq!(
        peer.load_project_data(&pid, "planning")
            .await
            .unwrap()
            .unwrap()
            .payload,
        payload
    );
    assert_eq!(peer.load_scene(&sid).await.unwrap(), original);
    assert_eq!(s.load_scene(&sid).await.unwrap(), original);
}
#[tokio::test]
async fn actual_foreign_missing_links_and_malformed_card_camouflage_refuse_atomically() {
    let (_d, s, pid, sid) = fixture().await;
    let good = planning(&sid);
    let saved = s
        .save_project_data(&pid, "planning", good.clone(), None)
        .await
        .unwrap();
    let foreign = s.create_novel("Foreign fixture").await.unwrap();
    let foreign_id = foreign.scene.id;
    let marker = s.sync_cursor().await.unwrap();
    let history = s
        .project_data_history(&pid, "planning")
        .await
        .unwrap()
        .len();
    let mut invalid = Vec::new();
    for target in [foreign_id.as_str(), "missing-scene"] {
        let mut v = good.clone();
        v["boards"][0]["cards"][0]["reference"]["id"] = json!(target);
        invalid.push(v);
        let mut v = good.clone();
        v["readerReveals"][0]["scene"]["id"] = json!(target);
        invalid.push(v);
        let mut v = good.clone();
        v["records"][0]["links"][0] = json!({"kind":"scene","id":target,"reference":null});
        invalid.push(v);
        let mut v = good.clone();
        v["records"][0]["links"][0] = card(target, Value::Null);
        invalid.push(v);
        let mut v = good.clone();
        v["boards"][0]["cards"][0] = json!({"kind":"scene","id":target,"reference":null});
        invalid.push(v);
        let mut v = good.clone();
        v["boards"][0]["cards"][0]["reference"] =
            json!({"kind":"scene","id":target,"reference":null});
        invalid.push(v);
    }
    let mut malformed = good.clone();
    malformed["boards"][0]["cards"][0] = card("not-an-owned-scene", Value::Null);
    malformed["boards"][0]["cards"][0]["x"] = json!("50");
    invalid.push(malformed);
    let mut malformed = good.clone();
    malformed["boards"][0]["cards"][0] = card("not-an-owned-scene", Value::Null);
    malformed["boards"][0]["cards"][0]["labels"] = json!([1]);
    invalid.push(malformed);
    for value in invalid {
        assert!(s
            .save_project_data(&pid, "planning", value, Some(saved.revision_id.clone()))
            .await
            .unwrap_err()
            .contains("scene"));
        assert_eq!(s.sync_cursor().await.unwrap(), marker);
        assert_eq!(
            s.project_data_history(&pid, "planning")
                .await
                .unwrap()
                .len(),
            history
        );
        assert_eq!(
            s.load_project_data(&pid, "planning")
                .await
                .unwrap()
                .unwrap(),
            saved
        );
    }
}
#[tokio::test]
async fn sparse_historical_cards_with_explicit_references_keep_independent_ids_without_rewriting_fields(
) {
    let (_d, s, pid, sid) = fixture().await;
    let mut payload = planning(&sid);
    payload["boards"][0]["cards"] = json!([{"id":sid,"kind":"scene","reference":{"kind":"scene","id":sid}},{"id":"legacy-card","reference":{"kind":"scene","id":sid},"body":"Legacy مِ 中文"}]);
    payload["boards"][0]["edges"][0]["toId"] = json!("legacy-card");
    let saved = s
        .save_project_data(&pid, "planning", payload.clone(), None)
        .await
        .unwrap();
    assert_eq!(saved.payload, payload);
    let imported = s
        .import_archive(&s.export_archive(&pid).await.unwrap())
        .await
        .unwrap();
    let mut expected = copied(&payload, &sid, &imported.scenes[0].id);
    expected["boards"][0]["cards"][1]["reference"]["id"] = json!(imported.scenes[0].id);
    assert_eq!(
        s.load_project_data(&imported.project.id, "planning")
            .await
            .unwrap()
            .unwrap()
            .payload,
        expected
    );
    assert!(expected["boards"][0]["cards"][0].get("x").is_none());
    assert!(expected["boards"][0]["cards"][1].get("kind").is_none());
    assert!(s.export_archive(&imported.project.id).await.is_ok());
}
#[tokio::test]
async fn forged_scene_card_reference_sync_rolls_back_projects_receipts_and_markers() {
    let (_d, s, pid, sid) = fixture().await;
    s.save_project_data(&pid, "planning", planning(&sid), None)
        .await
        .unwrap();
    let source = s.sync_cursor().await.unwrap();
    let page = s
        .pull_revisions(
            &pid,
            SyncCursor {
                epoch: source.epoch.clone(),
                sequence: 0,
            },
            1000,
        )
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let peer = Storage::open(&dir.path().join("replica.sqlite"))
        .await
        .unwrap();
    let before = peer.sync_cursor().await.unwrap();
    let receipts = peer.sync_receipt_count(&pid, &source.epoch).await.unwrap();
    for camouflage in [false, true] {
        let mut incoming = page.revisions.clone();
        let planning = incoming
            .iter_mut()
            .find(|r| r.operation == "planning")
            .unwrap();
        if camouflage {
            planning.payload["records"][0]["links"][0] = card("missing-scene", Value::Null);
        } else {
            planning.payload["boards"][0]["cards"][0]["reference"]["id"] = json!("missing-scene");
        }
        planning.checksum = format!(
            "{:x}",
            Sha256::digest(serde_json::to_string(&planning.payload).unwrap().as_bytes())
        );
        assert!(peer
            .ingest_revisions(
                &pid,
                &before.epoch,
                &source.epoch,
                s.sync_manifest(&pid).await.unwrap(),
                incoming
            )
            .await
            .is_err());
        assert_eq!(peer.sync_cursor().await.unwrap(), before);
        assert_eq!(
            peer.sync_receipt_count(&pid, &source.epoch).await.unwrap(),
            receipts
        );
        assert!(peer.open_project_index(&pid).await.is_err());
        assert!(peer
            .load_project_data(&pid, "planning")
            .await
            .unwrap()
            .is_none());
    }
}
