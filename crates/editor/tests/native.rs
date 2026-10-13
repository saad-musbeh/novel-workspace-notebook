use novel_editor::{
    engine::NativeEditor,model::{
        self,TextModel
    }
};
use serde_json::{
    json,Value
};
fn document(text:&str)->Value{
    json!({
        "type":"doc","content":[{
            "type":"paragraph","content":[{
                "type":"text","text":text
            }]
        }]
    })
}
fn editor(text:&str)->(tempfile::TempDir,NativeEditor){
    let dir=tempfile::tempdir().unwrap();
    let e=NativeEditor::new(dir.path(),json!({
        "type":"open","scene":{
            "schemaVersion":1,"title":"Synthetic مِ 中文","document":document(text)
        }
    })).unwrap();
    (dir,e)
}
#[test]fn exact_original_shapes_restore_after_undo(){
    for original in [json!({
        "type":"doc"
    }),json!({
        "type":"doc","content":[]
    }),json!({
        "type":"doc","content":[{
            "type":"paragraph","attrs":{
            },"content":[]
        }]
    }),document("مِ 中文 e\u{301} 👩🏽‍🚀 𠀀 \u{e000}")]{
        let mut m=TextModel::new(original.clone()).unwrap();
        assert_eq!(m.document(),original);
        m.insert("X").unwrap();
        m.history(false);
        assert_eq!(m.document(),original);
    }
}
#[test]fn grapheme_delete_preserves_combining_marks_and_emoji(){
    for cluster in ["مِ","e\u{301}","👩🏽‍🚀","🇸🇦","𠀀","\u{e000}"]{
        let original=document(&format!("A{cluster}Z"));
        let mut m=TextModel::new(original.clone()).unwrap();
        m.move_to([0,1+model::len(cluster)],false);
        m.edit(&json!({
            "type":"key","key":"Backspace"
        })).unwrap();
        assert_eq!(m.flat_text(),"AZ");
        m.history(false);
        assert_eq!(m.document(),original);
        m.move_to([0,1],false);
        m.edit(&json!({
            "type":"key","key":"Delete"
        })).unwrap();
        assert_eq!(m.flat_text(),"AZ");
    }
}
#[test]fn immutable_neighbors_and_bounded_undo(){
    let blocks:Vec<_>=(0..1000).map(|i|json!({
        "type":"paragraph","content":[{
            "type":"text","text":format!("Paragraph {i} مِ 中文")
        }]
    })).collect();
    let mut m=TextModel::new(json!({
        "type":"doc","content":blocks
    })).unwrap();
    let neighbor=m.state.blocks[999].clone();
    for _ in 0..600{
        m.insert("X").unwrap();
    }assert!(std::sync::Arc::ptr_eq(&neighbor,&m.state.blocks[999]));
    assert_eq!(m.undo.len(),512);
    assert!(m.undo_trimmed);
    for _ in 0..512{
        m.history(false);
    }assert!(m.text(0).starts_with(&"X".repeat(88)));
}
#[test]fn multi_paragraph_replace_and_format_keep_exact_marks(){
    let mut m=TextModel::new(json!({
        "type":"doc","content":[{
            "type":"heading","attrs":{
                "lang":"ar","dir":"rtl","level":2,"lineSpacing":1
            },"content":[{
                "type":"text","text":"first مِ","marks":[{
                    "type":"underline"
                }]
            }]
        },{
            "type":"paragraph","content":[{
                "type":"text","text":"last 😀"
            }]
        }]
    })).unwrap();
    let original=m.document();
    m.state.anchor=[0,6];
    m.state.cursor=[1,5];
    m.insert("عَ\n𠀀").unwrap();
    assert_eq!(m.flat_text(),"first عَ\n𠀀😀");
    assert_eq!(m.state.blocks[1]["attrs"],json!({
        "lang":"ar","dir":"rtl","lineSpacing":1
    }));
    m.history(false);
    assert_eq!(m.document(),original);
}
#[test]fn literal_find_guarded_replace_and_unicode_toggle(){
    let mut m=TextModel::new(document("مِ 中文 / مِ 中文 / 0041")).unwrap();
    assert_eq!(m.find("مِ 中文").unwrap()["found"],true);
    assert_eq!(m.selection_text(),"مِ 中文");
    let original=m.document();
    assert_eq!(m.replace_selection("wrong","X").unwrap()["replaced"],false);
    assert_eq!(m.document(),original);
    m.replace_selection("مِ 中文","عَ").unwrap();
    m.history(false);
    assert_eq!(m.document(),original);
    m.move_to([0,model::len(&m.flat_text())],false);
    assert_eq!(m.unicode_toggle().unwrap()["text"],"A");
    assert_eq!(m.unicode_toggle().unwrap()["text"],"0041");
}
#[test]fn invalid_unicode_and_shapes_leave_original(){
    for code in ["D800","110000","1234567","0000","001B"]{
        let mut m=TextModel::new(document(code)).unwrap();
        m.move_to([0,code.len()],false);
        let before=m.document();
        assert_eq!(m.unicode_toggle().unwrap()["converted"],false);
        assert_eq!(m.document(),before);
        assert!(m.undo.is_empty());
    }for block in [json!({
        "type":"image"
    }),json!({
        "type":"paragraph","attrs":{
            "lineSpacing":true
        }
    }),json!({
        "type":"paragraph","content":[{
            "type":"text","text":"bad\u{0000}"
        }]
    })]{
        assert!(TextModel::new(json!({
            "type":"doc","content":[block]
        })).is_err());
    }
}
#[test]fn size_guard_and_failed_batch_are_atomic(){
    let (_dir,mut e)=editor("مِ 中文");
    let before=e.model.document();
    for operation in [json!({
        "type":"text","text":"X","width":4097
    }),json!({
        "type":"text","text":"X","top":true
    }),json!({
        "type":"text","text":"bad","inputs":["b","a"]
    }),json!({
        "type":"text","text":"ok\u{0000}","inputs":["ok","\u{0000}"]
    }),json!({
        "type":"command","command":"undo","deferRender":true
    }),json!({
        "type":"mouse","x":0,"y":0,"phase":"down","count":true
    })]{
        assert!(e.request(&operation).is_err());
        assert_eq!(e.model.document(),before);
        assert!(e.model.undo.is_empty());
    }e.model.max_document_bytes=serde_json::to_vec(&before).unwrap().len();
    assert!(e.request(&json!({
        "type":"text","text":"large"
    })).is_err());
    assert_eq!(e.model.document(),before);
}
#[test]fn ordered_batch_keeps_independent_undo_and_deferred_render(){
    let (_dir,mut e)=editor("Original");
    let inputs=["ي","س","م","ِ","中","👩🏽‍🚀"];
    let builds=e.layout.layout_builds;
    let frame=e.request(&json!({
        "type":"text","text":inputs.concat(),"inputs":inputs,"deferRender":true,"preview":true
    })).unwrap();
    assert_eq!(frame["deferred"],true);
    assert_eq!(e.layout.layout_builds,builds);
    let frame=e.request(&json!({
        "type":"snapshot"
    })).unwrap();
    assert!(frame["image"].as_str().unwrap().starts_with("data:image/png;base64,"));
    for _ in inputs{
        e.request(&json!({
            "type":"command","command":"undo","preview":true
        })).unwrap();
    }assert_eq!(e.model.document(),document("Original"));
}
#[test]fn checkpoint_roundtrip_retains_history_selection_and_private_permissions(){
    let (dir,mut e)=editor("Original مِ 中文");
    e.request(&json!({
        "type":"text","text":"X"
    })).unwrap();
    e.request(&json!({
        "type":"command","command":"selectAll"
    })).unwrap();
    let before=e.model.state.clone();
    e.request(&json!({
        "type":"command","command":"park"
    })).unwrap();
    let mut restored=NativeEditor::resume(dir.path()).unwrap();
    assert_eq!(restored.model.state,before);
    assert_eq!(restored.model.undo.len(),1);
    restored.request(&json!({
        "type":"command","command":"undo"
    })).unwrap();
    assert_eq!(restored.model.document(),document("Original مِ 中文"));
    #[cfg(unix)]{
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.path().join("checkpoint.json")).unwrap().permissions().mode()&0o777,0o600);
    }
}
#[test]fn forged_checkpoint_and_stale_paragraph_or_navigation_are_refused(){
    let (dir,mut e)=editor("Original");
    let capture=e.request(&json!({
        "type":"command","command":"captureParagraph"
    })).unwrap()["paragraphCapture"].clone();
    let nav=e.request(&json!({
        "type":"command","command":"captureNavigation"
    })).unwrap()["navigationCapture"].clone();
    e.request(&json!({
        "type":"text","text":"X"
    })).unwrap();
    let before=e.model.document();
    assert!(e.request(&json!({
        "type":"command","command":"swapParagraph","anchor":capture,"replacement":{
            "type":"paragraph"
        }
    })).is_err());
    assert!(e.request(&json!({
        "type":"command","command":"restoreNavigation","bookmark":nav
    })).is_err());
    assert_eq!(e.model.document(),before);
    e.request(&json!({
        "type":"command","command":"park"
    })).unwrap();
    let path=dir.path().join("checkpoint.json");
    let mut data:Value=serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    data["document"]=document("forged");
    std::fs::write(path,serde_json::to_vec(&data).unwrap()).unwrap();
    assert!(NativeEditor::resume(dir.path()).is_err());
}
#[test]fn viewport_only_paint_and_reflow_leave_text_and_undo_untouched(){
    let (_dir,mut e)=editor(&"English مِ 中文 👩🏽‍🚀 ".repeat(400));
    let before=e.model.document();
    let frame=e.request(&json!({
        "type":"refresh","top":300,"viewportHeight":200
    })).unwrap();
    let old_top=frame["top"].as_u64().unwrap();
    let resized=e.request(&json!({
        "type":"refresh","width":320,"viewportHeight":200
    })).unwrap();
    assert!(resized["top"].as_u64().unwrap()>old_top);
    assert!(e.layout.rendered_lines<20);
    assert_eq!(e.model.document(),before);
    assert!(e.model.undo.is_empty());
}
