use novel_storage::*;
use serde_json::{json,Value};

const METHOD:&str="Exact Unicode scalar/grapheme counts; whitespace-delimited tokens and CJK characters are separate measures, not comparable word counts.";
fn scene(text:&str)->Scene {Scene{schema_version:1,title:"Title is excluded 中文".into(),document:json!({"type":"doc","content":[{"type":"paragraph","attrs":{"lang":"ar","dir":"rtl"},"content":[{"type":"text","text":text,"marks":[{"type":"bold"},{"type":"underline"}]}]}]})}}
async fn fixture()->(tempfile::TempDir,Storage,Project,SceneRecord) {
    let d=tempfile::tempdir().unwrap();let s=Storage::open(&d.path().join("statistics.sqlite")).await.unwrap();let p=s.create_project("Saved counts 中文").await.unwrap();let c=s.create_chapter(&p.id,"Chapter 中文").await.unwrap();let r=s.create_scene(&c.id,scene("e\u{301} مِ 中文 𠀀 👩🏽‍🚀")).await.unwrap();(d,s,p,r)
}
async fn exchange(from:&Storage,to:&Storage,pid:&str) {let remote=from.sync_cursor().await.unwrap();let local=to.sync_cursor().await.unwrap();let page=from.pull_revisions(pid,SyncCursor{epoch:remote.epoch.clone(),sequence:0},100).await.unwrap();to.ingest_revisions(pid,&local.epoch,&remote.epoch,from.sync_manifest(pid).await.unwrap(),page.revisions).await.unwrap();}
#[tokio::test]
async fn exact_separate_unicode_counts_are_saved_read_only_and_exclude_titles_deleted_and_planning() {
    let (_d,s,p,r)=fixture().await;
    let deleted=s.create_scene(&r.chapter_id,scene("DELETED 中文 中文")).await.unwrap();s.delete_scene(&deleted.id,&deleted.revision_id).await.unwrap();
    s.save_project_data(&p.id,"planning",json!({"schemaVersion":1,"notes":"PLANNING 中文"}),None).await.unwrap();
    let marker=s.sync_cursor().await.unwrap();let stats=s.statistics(&p.id).await.unwrap();
    assert_eq!(stats,Statistics{scenes:1,unicode_scalars:15,graphemes:10,whitespace_words:5,cjk_characters:3,method:METHOD.into()});
    assert_eq!(s.sync_cursor().await.unwrap(),marker);assert_eq!(s.load_scene(&r.id).await.unwrap().scene,r.scene);
    let mut next=r.scene.clone();next.document["content"].as_array_mut().unwrap().push(json!({"type":"paragraph","content":[{"type":"text","text":"A"},{"type":"hardBreak"},{"type":"text","text":"B"}]}));
    s.save_scene(&r.id,next,Some(r.revision_id)).await.unwrap();let stats=s.statistics(&p.id).await.unwrap();
    assert_eq!((stats.unicode_scalars,stats.graphemes,stats.whitespace_words,stats.cjk_characters),(20,15,7,3));
}
#[tokio::test]
async fn unrelated_alphabet_translation_and_other_project_conflicts_do_not_block_counts() {
    let (_d,a,p,r)=fixture().await;let d=tempfile::tempdir().unwrap();let b=Storage::open(&d.path().join("peer.sqlite")).await.unwrap();exchange(&a,&b,&p.id).await;
    for store in [&a,&b] {
        store.save_project_data(&p.id,"writingSystems",json!({"schemaVersion":1,"systems":[],"assignments":[]}),None).await.unwrap();
        store.save_project_data(&p.id,"sceneTranslations",json!({"schemaVersion":1,"drafts":[]}),None).await.unwrap();
    }
    exchange(&b,&a,&p.id).await;
    assert!(a.alphabet_export_snapshot(&p.id).await.is_err());
    let other=a.create_project("Other").await.unwrap();let c=a.create_chapter(&other.id,"Other").await.unwrap();let old=a.create_scene(&c.id,r.scene.clone()).await.unwrap();a.save_scene(&old.id,old.scene.clone(),Some(old.revision_id.clone())).await.unwrap();a.preserve_conflict(&old.id,old.scene.clone(),&old.revision_id).await.unwrap();
    let marker=a.sync_cursor().await.unwrap();assert_eq!(a.statistics(&p.id).await.unwrap().unicode_scalars,15);assert_eq!(a.sync_cursor().await.unwrap(),marker);
    assert!(a.statistics(&other.id).await.is_err());
}
#[tokio::test]
async fn scene_branch_tombstone_and_hierarchy_conflicts_refuse_without_writes() {
    for kind in ["scene","delete","chapter","project"] {
        let (_d,a,p,r)=fixture().await;let d=tempfile::tempdir().unwrap();let b=Storage::open(&d.path().join("peer.sqlite")).await.unwrap();exchange(&a,&b,&p.id).await;
        match kind {
            "scene"=>{a.save_scene(&r.id,scene("Current"),Some(r.revision_id.clone())).await.unwrap();b.save_scene(&r.id,scene("Other"),Some(r.revision_id)).await.unwrap();},
            "delete"=>{a.delete_scene(&r.id,&r.revision_id).await.unwrap();b.delete_scene(&r.id,&r.revision_id).await.unwrap();},
            "chapter"=>{a.rename_chapter(&r.chapter_id,"First").await.unwrap();b.rename_chapter(&r.chapter_id,"Second").await.unwrap();},
            _=>{a.rename_project(&p.id,"First").await.unwrap();b.rename_project(&p.id,"Second").await.unwrap();}
        }
        exchange(&b,&a,&p.id).await;let marker=a.sync_cursor().await.unwrap();assert!(a.statistics(&p.id).await.unwrap_err().contains("Conflict"));assert_eq!(a.sync_cursor().await.unwrap(),marker);
    }
}
#[tokio::test]
async fn corrupt_scene_or_placement_checksum_schema_and_foreign_projection_refuse() {
    for fault in ["checksum","schema","placement","foreign"] {
        let (d,s,p,r)=fixture().await;let pool=sqlx::SqlitePool::connect(&format!("sqlite://{}",d.path().join("statistics.sqlite").display())).await.unwrap();
        sqlx::query("DROP TRIGGER immutable_revision_update").execute(&pool).await.unwrap();
        match fault {
            "checksum"=>{sqlx::query("UPDATE revision_envelope SET checksum='corrupt' WHERE id=?").bind(&r.revision_id).execute(&pool).await.unwrap();},
            "schema"=>{sqlx::query("UPDATE revision_envelope SET schema_version=2 WHERE id=?").bind(&r.revision_id).execute(&pool).await.unwrap();},
            "placement"=>{sqlx::query("UPDATE revision_envelope SET checksum='corrupt' WHERE entity_id=? AND entity_type='scenePlacement'").bind(&r.id).execute(&pool).await.unwrap();},
            _=>{let foreign=s.create_project("Foreign").await.unwrap();let c=s.create_chapter(&foreign.id,"Foreign").await.unwrap();let other=s.create_scene(&c.id,scene("Foreign")).await.unwrap();sqlx::query("UPDATE manuscript_scene SET head=? WHERE id=?").bind(other.revision_id).bind(&r.id).execute(&pool).await.unwrap();}
        }
        pool.close().await;let marker=s.sync_cursor().await.unwrap();assert!(s.statistics(&p.id).await.is_err());assert_eq!(s.sync_cursor().await.unwrap(),marker);
    }
}
#[tokio::test]
async fn missing_project_refuses_and_empty_project_has_zero_counts() {
    let (_d,s,_,_)=fixture().await;assert!(s.statistics("not-a-project").await.is_err());let p=s.create_project("Empty").await.unwrap();let stats=s.statistics(&p.id).await.unwrap();assert_eq!((stats.scenes,stats.unicode_scalars,stats.graphemes,stats.whitespace_words,stats.cjk_characters),(0,0,0,0,0));assert_eq!(stats.method,METHOD);
}
#[tokio::test]
async fn deferred_sole_chapter_order_requires_explicit_ack_but_new_members_are_ordinary_additions() {
    let (_d,s,p,first)=fixture().await;let second=s.create_scene(&first.chapter_id,scene("Second")).await.unwrap();
    s.reorder_scenes(&first.chapter_id,vec![first.id.clone(),second.id.clone()]).await.unwrap();
    s.create_scene(&first.chapter_id,scene("Added after saved order")).await.unwrap();assert_eq!(s.statistics(&p.id).await.unwrap().scenes,3);
    s.move_scene(&second.id,&first.chapter_id,0).await.unwrap();let marker=s.sync_cursor().await.unwrap();assert!(s.statistics(&p.id).await.unwrap_err().contains("Scene and chapter conflicts"));assert_eq!(s.sync_cursor().await.unwrap(),marker);
    let capture=s.metadata_resolution_snapshot(&p.id,&first.chapter_id,"chapter").await.unwrap();assert_eq!(capture.heads.len(),1);
    s.resolve_metadata_checked(&capture,capture.projection.clone()).await.unwrap();let marker=s.sync_cursor().await.unwrap();assert_eq!(s.statistics(&p.id).await.unwrap().scenes,3);assert_eq!(s.sync_cursor().await.unwrap(),marker);assert_eq!(s.load_scene(&first.id).await.unwrap().revision_id,first.revision_id);assert_eq!(s.load_scene(&second.id).await.unwrap().revision_id,second.revision_id);
}
#[tokio::test]
async fn total_saved_body_capture_over_64_mib_refuses_without_markers_or_source_changes() {
    let (_d,s,p,r)=fixture().await;let large=scene(&"x".repeat(1_950_000));
    for _ in 0..35 {s.create_scene(&r.chapter_id,large.clone()).await.unwrap();}
    let marker=s.sync_cursor().await.unwrap();assert!(s.statistics(&p.id).await.unwrap_err().contains("64 MiB"));assert_eq!(s.sync_cursor().await.unwrap(),marker);assert_eq!(s.load_scene(&r.id).await.unwrap().scene,r.scene);
}

#[test]
fn statistics_wire_shape_preserves_existing_count_names() {
    let value:Value=serde_json::to_value(Statistics{scenes:1,unicode_scalars:15,graphemes:10,whitespace_words:5,cjk_characters:3,method:METHOD.into()}).unwrap();
    assert_eq!(value,json!({"scenes":1,"unicodeScalars":15,"graphemes":10,"whitespaceWords":5,"cjkCharacters":3,"method":METHOD}));
}
