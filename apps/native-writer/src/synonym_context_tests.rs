use super::*;
fn document()->Value {json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar","lineSpacing":1.5},"content":[{"type":"text","text":"مِ e\u{301} 𠀀 ","marks":[{"type":"bold"}]},{"type":"text","text":"bank","marks":[{"type":"italic"},{"type":"underline"}]},{"type":"hardBreak"},{"type":"text","text":"中文 bank 👩🏽‍🚀","marks":[{"type":"bold"}]}]}]})}
#[test]
fn exact_selected_unicode_occurrence_preserves_rich_runs_and_does_not_map_literal_alternative() {
    let d=document();let prefix="مِ e\u{301} 𠀀 ".chars().count();
    let (index,block)=replacement(&d,&json!({"cursor":[0,prefix+4],"anchor":[0,prefix]}),"bank","shore e\u{301} 𠀀").unwrap();assert_eq!(index,0);assert_eq!(block["attrs"],d["content"][0]["attrs"]);assert_eq!(block["content"][0],d["content"][0]["content"][0]);assert_eq!(block["content"][1]["marks"],d["content"][0]["content"][1]["marks"]);assert_eq!(block["content"][2],json!({"type":"hardBreak"}));assert!(novel_storage::plain_text(&block).ends_with("中文 bank 👩🏽‍🚀"));assert_eq!(novel_storage::plain_text(&d),"مِ e\u{301} 𠀀 bank\n中文 bank 👩🏽‍🚀");
}
#[test]
fn wrong_case_normalization_caret_cross_paragraph_and_out_of_range_refuse() {
    let d=document();let prefix="مِ e\u{301} 𠀀 ".chars().count();let selection=json!({"cursor":[0,prefix+4],"anchor":[0,prefix]});
    for term in ["BANK","bank ","other"]{assert!(replacement(&d,&selection,term,"shore").is_err());}
    for selection in [json!({"cursor":[0,0],"anchor":[0,0]}),json!({"cursor":[1,0],"anchor":[0,0]}),json!({"cursor":[0,999],"anchor":[0,0]}),json!({"cursor":[0,true],"anchor":[0,0]})]{assert!(replacement(&d,&selection,"bank","shore").is_err());}
    assert!(replacement(&d,&json!({"cursor":[0,6],"anchor":[0,3]}),"é ","shore").is_err());
    assert!(replacement(&d,&selection,"bank","shore\nriver").is_err());
}
#[test]
fn full_rich_document_hash_changes_even_when_plain_quote_is_unchanged() {
    let d=document();let before=hash(&d);let mut changed=d.clone();changed["content"][0]["content"][1]["marks"]=json!([{"type":"bold"}]);assert_ne!(hash(&changed),before);assert_eq!(novel_storage::plain_text(&changed),novel_storage::plain_text(&d));
}
