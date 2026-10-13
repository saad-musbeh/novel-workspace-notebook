use novel_storage::*;
use serde_json::json;
use sha2::{
    Digest,
    Sha256
};
fn scene(text:&str)->Scene{
    Scene{
        schema_version:1,
        title:"Scene".into(),
        document:json!({
            "type":"doc","content":[{
                "type":"paragraph","content":[{
                    "type":"text","text":text
                }
                ]
            }
            ]
        }
        )
    }
}
fn hash(data:&[u8])->String{
    format!("{:x}",Sha256::digest(data))
}
async fn store()->(tempfile::TempDir,Storage){
    let d=tempfile::tempdir().unwrap();
    let s=Storage::open(&d.path().join("replica.sqlite")).await.unwrap();
    (d,s)
}
async fn initial(s:&Storage)->(String,String){
    let p=s.create_project("Replica").await.unwrap();
    let c=s.create_chapter(&p.id,"Chapter").await.unwrap();
    let sc=s.create_scene(&c.id,scene("ancestor مَ 世界 😀")).await.unwrap();
    (p.id,sc.id)
}
async fn exchange(from:&Storage,to:&Storage,pid:&str,after:i64)->IngestResult{
    let cursor=from.sync_cursor().await.unwrap();
    let page=from.pull_revisions(pid,SyncCursor{
        epoch:cursor.epoch.clone(),sequence:after
    },
    1000).await.unwrap();
    let local=to.sync_cursor().await.unwrap();
    to.ingest_revisions(pid,&local.epoch,&cursor.epoch,from.open_project(pid).await.unwrap(),page.revisions).await.unwrap()
}
#[tokio::test]
async fn single_asset_reads_and_sql_chunks_preserve_exact_bytes_and_project_bounds(){
    use sqlx::Connection;
    let(directory,s)=store().await;
    let(pid,_)=initial(&s).await;
    let(other,_)=initial(&s).await;
    let content=(0..300_019).map(|n|(n%251) as u8).collect::<Vec<_>>();
    let asset=ProjectAsset{id:"bounded-asset".into(),path:"assets/bounded.txt".into(),mime_type:"text/plain".into(),checksum:hash(&content),content};
    s.put_asset(&pid,asset.clone()).await.unwrap();
    assert_eq!(s.load_asset(&pid,&asset.id).await.unwrap(),Some(asset.clone()));
    assert!(s.load_asset(&other,&asset.id).await.unwrap().is_none());
    assert!(s.load_asset(&pid,"missing-asset").await.unwrap().is_none());
    assert!(s.load_asset("../invalid",&asset.id).await.is_err());
    for (offset,limit) in [(0,1),(1,257),(251,262144),(262144,262144),(asset.content.len(),1)]{
        let chunk=s.read_asset_chunk(&pid,&asset.id,offset,limit).await.unwrap();
        assert_eq!(chunk.asset_id,asset.id);
        assert_eq!(chunk.checksum,asset.checksum);
        assert_eq!(chunk.offset,offset);
        assert_eq!(chunk.total_size,asset.content.len());
        assert_eq!(chunk.bytes,asset.content[offset..offset.saturating_add(limit).min(asset.content.len())]);
        let encoded=serde_json::to_value(&chunk).unwrap();
        assert_eq!(encoded["assetId"],asset.id);
        assert_eq!(encoded["totalSize"],asset.content.len());
    }
    for (offset,limit) in [(0,0),(0,262145),(asset.content.len()+1,1),(usize::MAX,1),(0,usize::MAX)]{
        assert!(s.read_asset_chunk(&pid,&asset.id,offset,limit).await.is_err());
    }
    assert!(s.read_asset_chunk(&other,&asset.id,0,1).await.is_err());
    assert!(s.read_asset_chunk(&pid,"missing-asset",0,1).await.is_err());

    // Corruption in an unrelated BLOB must not affect a bounded single-asset read.
    let mut raw=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(directory.path().join("replica.sqlite"))).await.unwrap();
    sqlx::query("INSERT INTO project_asset(project_id,id,path,mime_type,content,checksum) VALUES(?,?,?,?,zeroblob(16777217),?)").bind(&pid).bind("oversized-unrelated").bind("assets/oversized.txt").bind("text/plain").bind("0".repeat(64)).execute(&mut raw).await.unwrap();
    assert_eq!(s.load_asset(&pid,&asset.id).await.unwrap(),Some(asset.clone()));
    assert_eq!(s.read_asset_chunk(&pid,&asset.id,0,1).await.unwrap().bytes,vec![0]);
    assert!(s.load_asset(&pid,"oversized-unrelated").await.is_err());
    assert!(s.read_asset_chunk(&pid,"oversized-unrelated",0,1).await.is_err());
    sqlx::query("INSERT INTO project_asset(project_id,id,path,mime_type,content,checksum) VALUES(?,?,?,?,?,?)").bind(&pid).bind("corrupt-hash").bind("assets/corrupt.txt").bind("text/plain").bind(&asset.content).bind("0".repeat(64)).execute(&mut raw).await.unwrap();
    assert!(s.load_asset(&pid,"corrupt-hash").await.is_err());
    // Chunk receipts do not rehash the whole BLOB; final destination verification
    // detects corruption against this expected checksum.
    let chunk=s.read_asset_chunk(&pid,"corrupt-hash",0,262144).await.unwrap();
    assert_eq!(chunk.checksum,"0".repeat(64));
    assert_eq!(chunk.bytes,asset.content[..262144]);
}
#[tokio::test]
async fn sync_receipts_are_project_epoch_scoped_atomic_and_retained_in_backups(){
    let(_a,a)=store().await;
    let(_b,b)=store().await;
    let(pid,_)=initial(&a).await;
    let(other,_)=initial(&a).await;
    let remote=a.sync_cursor().await.unwrap();
    let expected=a.pull_revisions(&pid,SyncCursor{epoch:remote.epoch.clone(),sequence:0},1000).await.unwrap().revisions.len() as i64;
    let other_expected=a.pull_revisions(&other,SyncCursor{epoch:remote.epoch.clone(),sequence:0},1000).await.unwrap().revisions.len() as i64;
    assert_eq!(b.sync_receipt_count(&pid,&remote.epoch).await.unwrap(),0);
    exchange(&a,&b,&pid,0).await;
    assert_eq!(b.sync_receipt_count(&pid,&remote.epoch).await.unwrap(),expected);
    exchange(&a,&b,&pid,0).await;
    assert_eq!(b.sync_receipt_count(&pid,&remote.epoch).await.unwrap(),expected);
    assert_eq!(b.sync_receipt_count(&other,&remote.epoch).await.unwrap(),0);
    exchange(&a,&b,&other,0).await;
    assert_eq!(b.sync_receipt_count(&other,&remote.epoch).await.unwrap(),other_expected);
    assert_eq!(b.sync_receipt_count(&pid,&remote.epoch).await.unwrap(),expected);

    let page=a.pull_revisions(&pid,SyncCursor{epoch:remote.epoch.clone(),sequence:0},1000).await.unwrap();
    let manifest=a.sync_manifest(&pid).await.unwrap();
    let local=b.sync_cursor().await.unwrap();
    let second_epoch="synthetic-second-remote-epoch";
    let mut corrupt=page.revisions.clone();
    corrupt[1].checksum="invalid".into();
    assert!(b.ingest_revisions(&pid,&local.epoch,second_epoch,manifest.clone(),corrupt).await.is_err());
    assert_eq!(b.sync_receipt_count(&pid,second_epoch).await.unwrap(),0);
    assert_eq!(b.sync_cursor().await.unwrap(),local);

    let directory=tempfile::tempdir().unwrap();
    let path=directory.path().join("before-duplicate-receipts.sqlite");
    b.backup(&path).await.unwrap();
    let result=b.ingest_revisions(&pid,&local.epoch,second_epoch,manifest.clone(),page.revisions.clone()).await.unwrap();
    assert_eq!((result.inserted,result.duplicates),(0,expected as usize));
    assert_eq!(b.sync_receipt_count(&pid,second_epoch).await.unwrap(),expected);
    assert_eq!(b.sync_receipt_count(&pid,&remote.epoch).await.unwrap(),expected);
    assert_eq!(b.sync_receipt_count(&other,second_epoch).await.unwrap(),0);
    b.ingest_revisions(&pid,&local.epoch,second_epoch,manifest,page.revisions).await.unwrap();
    assert_eq!(b.sync_receipt_count(&pid,second_epoch).await.unwrap(),expected);
    assert_eq!(b.sync_cursor().await.unwrap(),local);

    let restored=Storage::open(&path).await.unwrap();
    assert_eq!(restored.sync_cursor().await.unwrap(),local);
    assert_eq!(restored.sync_receipt_count(&pid,&remote.epoch).await.unwrap(),expected);
    assert_eq!(restored.sync_receipt_count(&pid,second_epoch).await.unwrap(),0);
    assert!(b.sync_receipt_count("../invalid",second_epoch).await.is_err());
    assert!(b.sync_receipt_count(&pid,"").await.is_err());
    b.rotate_epoch().await.unwrap();
    assert_eq!(b.sync_receipt_count(&pid,&remote.epoch).await.unwrap(),0);
    assert_eq!(b.sync_receipt_count(&pid,second_epoch).await.unwrap(),0);
}
#[tokio::test]
async fn two_replicas_preserve_concurrent_heads_retry_dedup_and_explicit_resolution(){
    let(_a,a)=store().await;
    let(_b,b)=store().await;
    let(pid,sid)=initial(&a).await;
    let baseline=a.sync_cursor().await.unwrap().sequence;
    let boot=exchange(&a,&b,&pid,0).await;
    let expected=a.pull_revisions(&pid,SyncCursor{epoch:a.sync_cursor().await.unwrap().epoch,sequence:0},1000).await.unwrap().revisions.len();
    assert_eq!(boot.inserted,expected);
    let retry=exchange(&a,&b,&pid,0).await;
    assert_eq!(retry.inserted,0);
    assert_eq!(retry.duplicates,expected);
    let initial=a.load_scene(&sid).await.unwrap();
    let left=a.save_scene(&sid,scene("writer A"),Some(initial.revision_id.clone())).await.unwrap();
    let right=b.save_scene(&sid,scene("writer B"),Some(initial.revision_id)).await.unwrap();
    exchange(&a,&b,&pid,baseline).await;
    exchange(&b,&a,&pid,baseline).await;
    assert_eq!(a.scene_heads(&sid).await.unwrap().revision_ids.len(),2);
    assert_eq!(b.scene_heads(&sid).await.unwrap().revision_ids.len(),2);
    assert_eq!(a.load_scene(&sid).await.unwrap(),left);
    assert_eq!(b.load_scene(&sid).await.unwrap(),right);
    let local=a.sync_cursor().await.unwrap().sequence;
    let merged=a.resolve_scene(&sid,scene("merged A + B"),&left.revision_id,vec![right.revision_id]).await.unwrap();
    assert_eq!(a.scene_heads(&sid).await.unwrap().revision_ids,vec![merged.revision_id.clone()]);
    exchange(&a,&b,&pid,local).await;
    assert_eq!(b.load_scene(&sid).await.unwrap(),merged);
    assert_eq!(a.history(&sid).await.unwrap().len(),4);
}
#[tokio::test]
async fn delete_edit_conflict_survives_archive_and_explicit_restore(){
    let(_a,a)=store().await;
    let(_b,b)=store().await;
    let(pid,sid)=initial(&a).await;
    exchange(&a,&b,&pid,0).await;
    let baseline=a.sync_cursor().await.unwrap().sequence;
    let ancestor=a.load_scene(&sid).await.unwrap();
    let deletion=a.delete_scene(&sid,&ancestor.revision_id).await.unwrap();
    assert!(a.open_project(&pid).await.unwrap().scenes.is_empty());
    assert!(a.save_scene(&sid,scene("accidental resurrection"),Some(ancestor.revision_id.clone())).await.is_err());
    b.save_scene(&sid,scene("offline edit"),Some(ancestor.revision_id)).await.unwrap();
    exchange(&b,&a,&pid,baseline).await;
    let heads=a.scene_heads(&sid).await.unwrap().revision_ids;
    assert_eq!(heads.len(),2);
    assert!(heads.contains(&deletion));
    assert_eq!(a.open_project(&pid).await.unwrap().scenes.len(),1);
    let restored_archive=a.import_archive(&a.export_archive(&pid).await.unwrap()).await.unwrap();
    assert_eq!(a.scene_heads(&restored_archive.scenes[0].id).await.unwrap().revision_ids.len(),2);
    let restored=a.restore_deleted_scene(&sid,scene("author reconciled deletion"),heads).await.unwrap();
    assert_eq!(a.scene_heads(&sid).await.unwrap().revision_ids,vec![restored.revision_id]);
    assert_eq!(a.conflicts(&sid).await.unwrap().len(),0);
}
#[tokio::test]
async fn lone_tombstone_roundtrip_never_silently_resurrects(){
    let(_d,s)=store().await;
    let(pid,sid)=initial(&s).await;
    let parent=s.load_scene(&sid).await.unwrap();
    s.save_project_data(&pid,"planning",json!({
        "schemaVersion":1,"issues":[{
            "id":"issue",
            "links":[{
                "kind":"scene","id":sid
            }
            ]
        }
        ]
    }
    ),None).await.unwrap();
    let deletion=s.delete_scene(&sid,&parent.revision_id).await.unwrap();
    assert_eq!(s.scene_heads(&sid).await.unwrap().revision_ids,vec![deletion]);
    let imported=s.import_archive(&s.export_archive(&pid).await.unwrap()).await.unwrap();
    assert!(imported.scenes.is_empty());
    assert_eq!(s.deleted_scenes(&imported.project.id).await.unwrap().len(),1);
    let data=s.load_project_data(&imported.project.id,"planning").await.unwrap().unwrap();
    assert!(data.payload["issues"][0]["links"][0]["id"].as_str().is_some());
}
#[tokio::test]
async fn corrupt_missing_parent_identity_reuse_and_stale_epochs_are_atomic(){
    let(_a,a)=store().await;
    let(_b,b)=store().await;
    let(pid,_)=initial(&a).await;
    let remote=a.sync_cursor().await.unwrap();
    let local=b.sync_cursor().await.unwrap();
    let manifest=a.open_project(&pid).await.unwrap();
    let page=a.pull_revisions(&pid,SyncCursor{
        epoch:remote.epoch.clone(),sequence:0
    },
    1000).await.unwrap();
    let mut bad=page.revisions.clone();
    bad[2].checksum="bad".into();
    assert!(b.ingest_revisions(&pid,&local.epoch,&remote.epoch,manifest.clone(),bad).await.is_err());
    assert!(b.list_projects().await.unwrap().is_empty());
    let mut bad=page.revisions.clone();
    bad[2].parents=vec!["unknown-parent".into()];
    assert!(b.ingest_revisions(&pid,&local.epoch,&remote.epoch,manifest.clone(),bad).await.is_err());
    assert!(b.list_projects().await.unwrap().is_empty());
    exchange(&a,&b,&pid,0).await;
    let mut reused=page.revisions;
    reused[2].payload=serde_json::to_value(scene("tampered identity")).unwrap();
    reused[2].checksum=hash(serde_json::to_string(&reused[2].payload).unwrap().as_bytes());
    assert!(b.ingest_revisions(&pid,&local.epoch,&remote.epoch,manifest.clone(),reused).await.is_err());
    let rotated=b.rotate_epoch().await.unwrap();
    assert_ne!(rotated.epoch,local.epoch);
    assert!(b.pull_revisions(&pid,local.clone(),100).await.is_err());
    assert!(b.ingest_revisions(&pid,&local.epoch,&remote.epoch,manifest,vec![]).await.is_err());
    assert!(b.pull_revisions(&pid,SyncCursor{
        epoch:rotated.epoch,sequence:999999
    },
    100).await.is_err());
}
#[tokio::test]
async fn verified_assets_resume_across_reopen_reject_gaps_corruption_and_retry_finalize(){
    let(d,s)=store().await;
    let(pid,_)=initial(&s).await;
    let bytes=b"synthetic font fixture only".to_vec();
    let spec=AssetTransferSpec{
        asset_id:"synthetic-font".into(),
        path:"assets/font.ttf".into(),
        mime_type:"font/ttf".into(),
        total_size:bytes.len(),
        checksum:hash(&bytes)
    };
    let transfer=s.begin_asset_transfer(&pid,spec.clone()).await.unwrap();
    assert!(s.finalize_asset_transfer(&pid,&transfer.transfer_id).await.is_err());
    assert!(s.append_asset_chunk(&pid,&transfer.transfer_id,5,vec![1]).await.is_err());
    let first=s.append_asset_chunk(&pid,&transfer.transfer_id,0,bytes[..8].to_vec()).await.unwrap();
    assert_eq!(first.received,8);
    assert_eq!(s.append_asset_chunk(&pid,&transfer.transfer_id,0,bytes[..8].to_vec()).await.unwrap(),first);
    assert!(s.append_asset_chunk(&pid,&transfer.transfer_id,0,vec![0;
    8]).await.is_err());
    s.close().await;
    let s=Storage::open(&d.path().join("replica.sqlite")).await.unwrap();
    assert_eq!(s.begin_asset_transfer(&pid,spec.clone()).await.unwrap(),first);
    s.append_asset_chunk(&pid,&transfer.transfer_id,8,bytes[8..].to_vec()).await.unwrap();
    let asset=s.finalize_asset_transfer(&pid,&transfer.transfer_id).await.unwrap();
    assert_eq!(asset.content,bytes);
    assert_eq!(s.finalize_asset_transfer(&pid,&transfer.transfer_id).await.unwrap(),asset);
    let complete=s.begin_asset_transfer(&pid,spec).await.unwrap();
    assert_eq!(complete.received,bytes.len());
    assert_eq!(s.finalize_asset_transfer(&pid,&complete.transfer_id).await.unwrap(),asset);
    let corrupt=s.begin_asset_transfer(&pid,AssetTransferSpec{
        asset_id:"bad-font".into(),path:"assets/bad.ttf".into(),mime_type:"font/ttf".into(),total_size:3,checksum:hash(&[1,2,3])
    }
    ).await.unwrap();
    s.append_asset_chunk(&pid,&corrupt.transfer_id,0,vec![9,9,9]).await.unwrap();
    assert!(s.finalize_asset_transfer(&pid,&corrupt.transfer_id).await.is_err());
    assert_eq!(s.list_assets(&pid).await.unwrap().len(),1);
}
#[tokio::test]
async fn generic_project_data_concurrency_requires_explicit_multiparent_resolution(){
    let(_a,a)=store().await;
    let(_b,b)=store().await;
    let(pid,_)=initial(&a).await;
    let initial=a.save_project_data(&pid,"settings",json!({
        "theme":"initial"
    }
    ),None).await.unwrap();
    exchange(&a,&b,&pid,0).await;
    let baseline=a.sync_cursor().await.unwrap().sequence;
    let left=a.save_project_data(&pid,"settings",json!({
        "theme":"left"
    }
    ),Some(initial.revision_id.clone())).await.unwrap();
    let right=b.save_project_data(&pid,"settings",json!({
        "theme":"right"
    }
    ),Some(initial.revision_id)).await.unwrap();
    exchange(&a,&b,&pid,baseline).await;
    exchange(&b,&a,&pid,baseline).await;
    assert_eq!(a.load_project_data(&pid,"settings").await.unwrap().unwrap(),left);
    assert_eq!(b.load_project_data(&pid,"settings").await.unwrap().unwrap(),right);
    let heads=a.project_data_heads(&pid,"settings").await.unwrap().revision_ids;
    assert_eq!(heads.len(),2);
    let merged=a.resolve_project_data(&pid,"settings",json!({
        "theme":"author merged"
    }
    ),heads).await.unwrap();
    assert_eq!(a.project_data_heads(&pid,"settings").await.unwrap().revision_ids,vec![merged.revision_id]);
}
#[tokio::test]
async fn chapter_order_and_title_conflicts_retain_local_projection_until_author_resolution(){let(_a,a)=store().await;let(_b,b)=store().await;let(pid,_)=initial(&a).await;let c2=a.create_chapter(&pid,"Second").await.unwrap();let c3=a.create_chapter(&pid,"Third").await.unwrap();exchange(&a,&b,&pid,0).await;let original=a.open_project(&pid).await.unwrap();let c1=original.chapters[0].id.clone();let baseline=a.sync_cursor().await.unwrap().sequence;a.reorder_chapters(&pid,vec![c2.id.clone(),c1.clone(),c3.id.clone()]).await.unwrap();b.reorder_chapters(&pid,vec![c3.id.clone(),c1.clone(),c2.id.clone()]).await.unwrap();exchange(&a,&b,&pid,baseline).await;exchange(&b,&a,&pid,baseline).await;assert_eq!(a.open_project(&pid).await.unwrap().chapters[0].id,c2.id);assert_eq!(b.open_project(&pid).await.unwrap().chapters[0].id,c3.id);let heads=a.metadata_heads(&pid,&pid,"project").await.unwrap().revision_ids;assert_eq!(heads.len(),2);assert!(a.rename_project(&pid,"Must resolve first").await.unwrap_err().starts_with("Conflict"));assert_eq!(a.open_project(&pid).await.unwrap().project.title,"Replica");let revision=a.resolve_metadata(&pid,&pid,"project",json!({"title":"Author resolved","order":[c1,c2.id,c3.id]}),heads).await.unwrap();assert_eq!(a.metadata_heads(&pid,&pid,"project").await.unwrap().revision_ids,vec![revision]);assert_eq!(a.open_project(&pid).await.unwrap().project.title,"Author resolved");assert!(a.import_archive(&a.export_archive(&pid).await.unwrap()).await.is_ok());}
#[tokio::test]
async fn fresh_replica_bootstrap_preserves_tombstones_and_retained_evidence(){let(_a,a)=store().await;let(_b,b)=store().await;let(pid,sid)=initial(&a).await;let first=a.load_scene(&sid).await.unwrap();a.save_project_data(&pid,"planning",json!({"schemaVersion":1,"issues":[{"id":"issue","links":[{"kind":"scene","id":sid}]}]}),None).await.unwrap();a.delete_scene(&sid,&first.revision_id).await.unwrap();assert!(a.open_project(&pid).await.unwrap().scenes.is_empty());let hub=a.sync_cursor().await.unwrap();let page=a.pull_revisions(&pid,SyncCursor{epoch:hub.epoch.clone(),sequence:0},1000).await.unwrap();let local=b.sync_cursor().await.unwrap();b.ingest_revisions(&pid,&local.epoch,&hub.epoch,a.sync_manifest(&pid).await.unwrap(),page.revisions).await.unwrap();assert!(b.open_project(&pid).await.unwrap().scenes.is_empty());assert_eq!(b.deleted_scenes(&pid).await.unwrap()[0].id,sid);assert!(b.load_project_data(&pid,"planning").await.unwrap().is_some());assert!(b.export_archive(&pid).await.is_ok());}

#[tokio::test]
async fn batched_metadata_reorder_then_rename_converges_with_full_leaf_snapshot(){
    let(_a,a)=store().await;let(_b,b)=store().await;let(pid,sid)=initial(&a).await;
    let c2=a.create_chapter(&pid,"Second").await.unwrap();
    exchange(&a,&b,&pid,0).await;
    let c1=a.open_project(&pid).await.unwrap().chapters[0].id.clone();
    let baseline=a.sync_cursor().await.unwrap().sequence;
    a.reorder_chapters(&pid,vec![c2.id.clone(),c1.clone()]).await.unwrap();
    a.rename_project(&pid,"Renamed after reorder").await.unwrap();
    exchange(&a,&b,&pid,baseline).await;
    assert_eq!(a.open_project(&pid).await.unwrap(),b.open_project(&pid).await.unwrap());
    let extra=a.create_scene(&c1,scene("Visible scene")).await.unwrap();
    let saved=a.load_scene(&sid).await.unwrap();a.delete_scene(&sid,&saved.revision_id).await.unwrap();
    a.reorder_scenes(&c1,vec![extra.id.clone()]).await.unwrap();
    assert_eq!(a.open_project(&pid).await.unwrap().scenes.len(),1);
    assert_eq!(a.deleted_scenes(&pid).await.unwrap()[0].id,sid);
}

#[tokio::test]
async fn choosing_deletion_acknowledges_all_conflicts_atomically_and_replicates(){
    let(_a,a)=store().await;let(_b,b)=store().await;let(pid,sid)=initial(&a).await;exchange(&a,&b,&pid,0).await;
    let baseline=a.sync_cursor().await.unwrap().sequence;let original=a.load_scene(&sid).await.unwrap();
    let edited=a.save_scene(&sid,scene("Retained conflicting draft"),Some(original.revision_id.clone())).await.unwrap();
    let deletion=b.delete_scene(&sid,&original.revision_id).await.unwrap();exchange(&b,&a,&pid,baseline).await;
    let count=a.changes(0,100).await.unwrap().len();
    assert!(a.resolve_scene_deletion(&sid,&original.revision_id,vec![deletion.clone()]).await.is_err());
    assert!(a.resolve_scene_deletion(&sid,&edited.revision_id,vec![original.revision_id.clone()]).await.is_err());
    assert!(a.resolve_scene_deletion(&sid,&edited.revision_id,vec![deletion.clone(),deletion.clone()]).await.is_err());
    assert!(a.resolve_scene(&sid,scene("Must not erase unacknowledged head"),&edited.revision_id,vec![original.revision_id]).await.is_err());
    assert_eq!(a.changes(0,100).await.unwrap().len(),count);
    let resolved=a.resolve_scene_deletion(&sid,&edited.revision_id,vec![deletion.clone()]).await.unwrap();
    assert_eq!(a.scene_heads(&sid).await.unwrap().revision_ids,vec![resolved.clone()]);
    assert!(a.open_project(&pid).await.unwrap().scenes.is_empty());
    assert_eq!(a.load_revision(&sid,&edited.revision_id).await.unwrap(),edited);
    let revision=a.load_entity_revision(&pid,&sid,"scene",&resolved).await.unwrap();
    assert_eq!(revision.entity_type,"tombstone");assert_eq!(revision.parents,vec![edited.revision_id,deletion]);
    let remote=a.sync_cursor().await.unwrap();let local=b.sync_cursor().await.unwrap();
    let page=a.pull_revisions(&pid,SyncCursor{epoch:remote.epoch.clone(),sequence:baseline},100).await.unwrap();
    b.ingest_revisions(&pid,&local.epoch,&remote.epoch,a.sync_manifest(&pid).await.unwrap(),page.revisions).await.unwrap();
    assert!(b.open_project(&pid).await.unwrap().scenes.is_empty());assert_eq!(b.scene_heads(&sid).await.unwrap().revision_ids,vec![resolved]);
}
