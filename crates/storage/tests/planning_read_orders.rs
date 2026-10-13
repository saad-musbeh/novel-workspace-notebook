use novel_storage::{Scene, Storage, SyncCursor};
use serde_json::json;

async fn store() -> (tempfile::TempDir, Storage) {
    let dir = tempfile::tempdir().unwrap();
    let storage = Storage::open(&dir.path().join("orders.sqlite"))
        .await
        .unwrap();
    (dir, storage)
}
fn scene(title: &str) -> Scene {
    Scene {
        schema_version: 1,
        title: title.into(),
        document: json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"Exact مِ e\u{301} 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"},{"type":"underline"}]}]}]}),
    }
}
async fn exchange(from: &Storage, to: &Storage, pid: &str, after: i64) {
    let epoch = from.sync_cursor().await.unwrap().epoch;
    let local = to.sync_cursor().await.unwrap().epoch;
    let page = from
        .pull_revisions(
            pid,
            SyncCursor {
                epoch: epoch.clone(),
                sequence: after,
            },
            100,
        )
        .await
        .unwrap();
    to.ingest_revisions(
        pid,
        &local,
        &epoch,
        from.sync_manifest(pid).await.unwrap(),
        page.revisions,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn settled_placement_does_not_make_deferred_single_head_order_safe_for_reader_cutoffs() {
    let (_a, primary) = store().await;
    let (_b, peer) = store().await;
    let project = primary
        .create_project("Deferred reader order")
        .await
        .unwrap();
    let a = primary.create_chapter(&project.id, "A").await.unwrap();
    let b = primary.create_chapter(&project.id, "B").await.unwrap();
    let c = primary.create_chapter(&project.id, "C").await.unwrap();
    let moved = primary
        .create_scene(&a.id, scene("Moving scene"))
        .await
        .unwrap();
    let first = primary.create_scene(&c.id, scene("First")).await.unwrap();
    let second = primary.create_scene(&c.id, scene("Second")).await.unwrap();
    exchange(&primary, &peer, &project.id, 0).await;
    let after = peer.sync_cursor().await.unwrap().sequence;
    let capture = primary
        .scene_arrangement_snapshot(&project.id, &moved.id)
        .await
        .unwrap();
    primary.arrange_scene(&capture, &b.id, 0).await.unwrap();
    let capture = peer
        .scene_arrangement_snapshot(&project.id, &moved.id)
        .await
        .unwrap();
    peer.arrange_scene(&capture, &c.id, 0).await.unwrap();
    peer.reorder_scenes(
        &c.id,
        vec![second.id.clone(), first.id.clone(), moved.id.clone()],
    )
    .await
    .unwrap();
    exchange(&peer, &primary, &project.id, after).await;
    let capture = primary
        .metadata_resolution_snapshot(&project.id, &moved.id, "scenePlacement")
        .await
        .unwrap();
    assert_eq!(capture.heads.len(), 2);
    primary
        .resolve_metadata_checked(&capture, json!({"chapterId":c.id,"position":0}))
        .await
        .unwrap();
    // Settle all competing heads and acknowledge the now-incompatible B order.
    let capture = primary
        .metadata_resolution_snapshot(&project.id, &a.id, "chapter")
        .await
        .unwrap();
    assert_eq!(capture.heads.len(), 2);
    primary
        .resolve_metadata_checked(&capture, capture.projection.clone())
        .await
        .unwrap();
    let capture = primary
        .metadata_resolution_snapshot(&project.id, &b.id, "chapter")
        .await
        .unwrap();
    assert_eq!(capture.heads.len(), 1);
    primary
        .resolve_metadata_checked(&capture, capture.projection.clone())
        .await
        .unwrap();
    let c_capture = primary
        .metadata_resolution_snapshot(&project.id, &c.id, "chapter")
        .await
        .unwrap();
    assert_eq!(c_capture.heads.len(), 1);
    let retained = primary
        .load_entity_revision(&project.id, &c.id, "chapter", &c_capture.heads[0])
        .await
        .unwrap()
        .payload;
    assert_eq!(retained["order"], json!([second.id, first.id, moved.id]));
    assert_ne!(retained["order"], c_capture.projection["order"]);
    let marker = primary.sync_cursor().await.unwrap();
    let error = primary
        .planning_read_snapshot(&project.id)
        .await
        .unwrap_err();
    assert!(error.contains("Scene and chapter conflicts"), "{error}");
    assert_eq!(primary.sync_cursor().await.unwrap(), marker);
    assert_eq!(
        primary
            .metadata_heads(&project.id, &c.id, "chapter")
            .await
            .unwrap()
            .revision_ids,
        c_capture.heads
    );
    primary
        .resolve_metadata_checked(&c_capture, retained)
        .await
        .unwrap();
    let marker = primary.sync_cursor().await.unwrap();
    let snapshot = primary.planning_read_snapshot(&project.id).await.unwrap();
    let order: Vec<_> = snapshot
        .index
        .scenes
        .iter()
        .filter(|s| s.chapter_id == c.id)
        .map(|s| s.id.clone())
        .collect();
    assert_eq!(
        order,
        [second.id.clone(), first.id.clone(), moved.id.clone()]
    );
    assert_eq!(primary.sync_cursor().await.unwrap(), marker);
    for original in [moved, first, second] {
        assert_eq!(
            primary
                .load_project_scene(&project.id, &original.id)
                .await
                .unwrap()
                .scene,
            original.scene
        );
        assert_eq!(
            primary.load_scene(&original.id).await.unwrap().revision_id,
            original.revision_id
        );
    }
}

#[tokio::test]
async fn new_unlisted_scenes_are_allowed_but_foreign_listed_members_require_acknowledgement() {
    let (_dir, storage) = store().await;
    let project = storage.create_project("New additions").await.unwrap();
    let chapter = storage
        .create_chapter(&project.id, "Chapter")
        .await
        .unwrap();
    let other = storage
        .create_chapter(&project.id, "Other chapter")
        .await
        .unwrap();
    let first = storage
        .create_scene(&chapter.id, scene("First"))
        .await
        .unwrap();
    let second = storage
        .create_scene(&chapter.id, scene("Second"))
        .await
        .unwrap();
    storage
        .reorder_scenes(&chapter.id, vec![second.id.clone(), first.id.clone()])
        .await
        .unwrap();
    let addition = storage
        .create_scene(&chapter.id, scene("New unlisted addition"))
        .await
        .unwrap();
    // Even a new member inserted before the retained members is safe: their
    // relative order is unchanged and no older envelope claims its position.
    storage
        .move_scene(&addition.id, &chapter.id, 0)
        .await
        .unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    assert!(storage.planning_read_snapshot(&project.id).await.is_ok());
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    storage.move_scene(&first.id, &other.id, 0).await.unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    assert!(storage
        .planning_read_snapshot(&project.id)
        .await
        .unwrap_err()
        .contains("retained chapter order"));
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    let capture = storage
        .metadata_resolution_snapshot(&project.id, &chapter.id, "chapter")
        .await
        .unwrap();
    storage
        .resolve_metadata_checked(&capture, capture.projection.clone())
        .await
        .unwrap();
    assert!(storage.planning_read_snapshot(&project.id).await.is_ok());
    assert_eq!(
        storage.load_scene(&first.id).await.unwrap().revision_id,
        first.revision_id
    );
    assert_eq!(
        storage.load_scene(&addition.id).await.unwrap().scene,
        addition.scene
    );
}

#[tokio::test]
async fn deleted_members_still_participate_in_retained_relative_order() {
    let (_dir, storage) = store().await;
    let project = storage
        .create_project("Deletion does not erase order")
        .await
        .unwrap();
    let chapter = storage
        .create_chapter(&project.id, "Chapter")
        .await
        .unwrap();
    let first = storage
        .create_scene(&chapter.id, scene("Hidden deleted scene"))
        .await
        .unwrap();
    let second = storage
        .create_scene(&chapter.id, scene("Live scene"))
        .await
        .unwrap();
    storage
        .reorder_scenes(&chapter.id, vec![first.id.clone(), second.id.clone()])
        .await
        .unwrap();
    storage
        .delete_scene(&first.id, &first.revision_id)
        .await
        .unwrap();
    let marker = storage.sync_cursor().await.unwrap();
    let view = storage.planning_read_snapshot(&project.id).await.unwrap();
    assert_eq!(view.index.scenes.len(), 1);
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    // Placement-only order change includes the deleted row in normalization.
    storage
        .move_scene(&second.id, &chapter.id, 0)
        .await
        .unwrap();
    let capture = storage
        .metadata_resolution_snapshot(&project.id, &chapter.id, "chapter")
        .await
        .unwrap();
    assert_eq!(capture.projection["order"], json!([second.id, first.id]));
    let marker = storage.sync_cursor().await.unwrap();
    assert!(storage
        .planning_read_snapshot(&project.id)
        .await
        .unwrap_err()
        .contains("retained chapter order"));
    assert_eq!(storage.sync_cursor().await.unwrap(), marker);
    storage
        .resolve_metadata_checked(&capture, capture.projection.clone())
        .await
        .unwrap();
    assert!(storage.planning_read_snapshot(&project.id).await.is_ok());
    assert_eq!(
        storage.load_scene(&second.id).await.unwrap().scene,
        second.scene
    );
    assert_eq!(
        storage
            .load_entity_revision(&project.id, &first.id, "scene", &first.revision_id)
            .await
            .unwrap()
            .payload,
        serde_json::to_value(first.scene).unwrap()
    );
}
