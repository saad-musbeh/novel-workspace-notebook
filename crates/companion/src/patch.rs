//! Pure revision-bound rich-text edits. The caller owns current-head CAS and durable revision creation.
use novel_storage::{plain_text,validate,Scene};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use unicode_segmentation::UnicodeSegmentation;
fn hash(text:&str)->String {format!("{:x}",Sha256::digest(text.as_bytes()))}
fn byte_at(text:&str,offset:usize)->Option<usize> {let mut units=0;for (byte,c) in text.char_indices(){if units==offset{return Some(byte)}units+=c.len_utf16();}if units==offset{Some(text.len())}else{None}}
fn slice(text:&str,start:usize,end:usize)->Result<&str,String>{if start>end{return Err("Invalid text range.".into())}let a=byte_at(text,start).ok_or("The edit splits a Unicode character.")?;let b=byte_at(text,end).ok_or("The edit splits a Unicode character.")?;Ok(&text[a..b])}
fn offset(value:&Value)->Result<usize,String>{usize::try_from(value.as_u64().ok_or("Invalid evidence offsets.")?).map_err(|_|"Evidence offsets exceed this platform's limit.".into())}
fn nodes(text:&str,template:&Value)->Vec<Value>{let mut result=Vec::new();for (i,segment) in text.split('\n').enumerate(){if i>0{result.push(json!({"type":"hardBreak"}));}if !segment.is_empty(){let mut node=template.clone();node["type"]=json!("text");node["text"]=json!(segment);if let Some(object)=node.as_object_mut(){object.remove("content");}result.push(node);}}result}
fn replace_block(block:&mut Value,start:usize,end:usize,replacement:&str)->Result<(),String>{
 let children=block.get("content").and_then(Value::as_array).ok_or("This block has no editable text runs.")?;let mut result=Vec::new();let mut position=0;let mut inserted=false;
 for node in children {
  let text=match node["type"].as_str(){Some("text")=>node["text"].as_str().ok_or("Invalid text run.")?,Some("hardBreak")=>"\n",_=>return Err("Replacement supports text and line-break runs within one paragraph only.".into())};let length=text.encode_utf16().count();let next=position+length;
  if next<=start||position>=end{result.push(node.clone());}else{
   if start>position{let prefix=slice(text,0,start-position)?;result.extend(nodes(prefix,node));}
   if !inserted{let template=if node["type"]=="text"{node.clone()}else{json!({"type":"text"})};result.extend(nodes(replacement,&template));inserted=true;}
   if end<next{let suffix=slice(text,end-position,length)?;result.extend(nodes(suffix,node));}
  }position=next;
 }
 if !inserted{return Err("Evidence does not intersect an editable text run.".into())}block["content"]=json!(result);Ok(())
}
pub fn apply(scene:&Scene,patch:&Value)->Result<Scene,String>{
 validate(scene)?;let text=plain_text(&scene.document);let expected=patch["source"]["hash"].as_str().ok_or("Missing expected source hash.")?;
 if expected!=hash(&text){return Err("The saved scene hash changed. Run a new review before applying this edit.".into())}
 let anchor=&patch["anchor"];
 for key in ["sceneId","revisionId","hash"]{if anchor[key]!=patch["source"][key]||!anchor[key].is_string(){return Err("Evidence does not identify the expected scene revision.".into())}}
 if anchor["offsetUnit"]!="utf16"{return Err("Unsupported evidence offset unit.".into())}let start=offset(&anchor["start"])?;let end=offset(&anchor["end"])?;
 if start>=end{return Err("A replacement needs a nonempty exact evidence quote.".into())}let quote=slice(&text,start,end)?;
 if anchor["quote"].as_str()!=Some(quote){return Err("Quoted evidence no longer matches the saved scene.".into())}
 let start_byte=byte_at(&text,start).ok_or("Invalid character boundary.")?;let end_byte=byte_at(&text,end).ok_or("Invalid character boundary.")?;
 let mut start_ok=false;let mut end_ok=false;for byte in text.grapheme_indices(true).map(|(byte,_)|byte).chain(std::iter::once(text.len())){start_ok|=byte==start_byte;end_ok|=byte==end_byte;}
 if !start_ok||!end_ok{return Err("The edit splits a grapheme or combining sequence.".into())}
 let prefix=anchor["prefix"].as_str().ok_or("Missing evidence prefix.")?;let suffix=anchor["suffix"].as_str().ok_or("Missing evidence suffix.")?;
 if prefix.encode_utf16().count()>80||suffix.encode_utf16().count()>80||!text[..start_byte].ends_with(prefix)||!text[end_byte..].starts_with(suffix){return Err("Evidence context no longer matches the saved scene.".into())}
 let replacement=patch["replacement"].as_str().ok_or("Missing proposed replacement.")?;if replacement.encode_utf16().count()>10000{return Err("Replacement exceeds its supported size.".into())}
 let mut edited=scene.clone();let blocks=edited.document.get_mut("content").and_then(Value::as_array_mut).ok_or("Missing document blocks.")?;let mut position=0;let mut applied=false;
 for block in blocks {let length=plain_text(block).encode_utf16().count();if start>=position&&end<=position+length{if !matches!(block["type"].as_str(),Some("paragraph"|"heading")){return Err("Unsupported editable block.".into())}replace_block(block,start-position,end-position,replacement)?;applied=true;break;}position+=length+2;}
 if !applied{return Err("This edit crosses paragraph boundaries. Review and edit those paragraphs manually; formatting was preserved.".into())}validate(&edited)?;
 let expected_text=format!("{}{}{}",&text[..start_byte],replacement,&text[end_byte..]);if plain_text(&edited.document)!=expected_text{return Err("Replacement could not preserve the document projection.".into())}Ok(edited)
}
#[cfg(test)]mod tests{
 use super::*;
 fn scene()->Scene{Scene{schema_version:1,title:"Synthetic".into(),document:json!({"type":"doc","content":[{"type":"paragraph","attrs":{"id":"block-1","lang":"ar","dir":"rtl"},"content":[{"type":"text","text":"مِ "},{"type":"text","text":"bold","marks":[{"type":"bold"}]},{"type":"text","text":" plain 😀","marks":[{"type":"italic"}]}]},{"type":"paragraph","attrs":{"lang":"zh-Hans","dir":"auto"},"content":[{"type":"text","text":"中文"}]}]})}}
 fn patch(scene:&Scene,start:usize,end:usize,replacement:&str)->Value{let text=plain_text(&scene.document);let source=json!({"sceneId":"s1","revisionId":"r1","hash":hash(&text)});json!({"source":source,"anchor":{"sceneId":"s1","revisionId":"r1","hash":hash(&text),"offsetUnit":"utf16","start":start,"end":end,"quote":slice(&text,start,end).unwrap_or("invalid"),"prefix":"","suffix":""},"replacement":replacement})}
 #[test]fn replaces_across_runs_and_preserves_marks_and_block_attributes(){let original=scene();let edited=apply(&original,&patch(&original,5,10,"NEW")).unwrap();assert_eq!(plain_text(&edited.document),"مِ boNEWain 😀\n\n中文");let block=&edited.document["content"][0];assert_eq!(block["attrs"],original.document["content"][0]["attrs"]);assert_eq!(block["content"][2]["marks"][0]["type"],"bold");assert_eq!(block["content"][3]["marks"][0]["type"],"italic");assert_eq!(edited.document["content"][1],original.document["content"][1]);}
 #[test]fn rejects_arabic_combining_and_surrogate_splits(){let original=scene();assert!(apply(&original,&patch(&original,0,1,"x")).unwrap_err().contains("grapheme"));assert!(apply(&original,&patch(&original,14,15,"x")).is_err());}
 #[test]fn replaces_complete_arabic_and_emoji_graphemes(){let original=scene();assert!(plain_text(&apply(&original,&patch(&original,0,2,"عُ")).unwrap().document).starts_with("عُ "));assert!(plain_text(&apply(&original,&patch(&original,14,16,"👩‍💻")).unwrap().document).contains("👩‍💻"));}
 #[test]fn rejects_stale_hash_wrong_quote_context_and_units(){let original=scene();let base=patch(&original,3,7,"strong");for (key,value) in [("quote",json!("fake")),("prefix",json!("fake")),("offsetUnit",json!("bytes")),("revisionId",json!("r2"))]{let mut changed=base.clone();changed["anchor"][key]=value;assert!(apply(&original,&changed).is_err());}let mut changed=base;changed["source"]["hash"]=json!("0".repeat(64));assert!(apply(&original,&changed).unwrap_err().contains("hash"));}
 #[test]fn rejects_paragraph_crossing_without_flattening(){let original=scene();let text=plain_text(&original.document);let end=text.encode_utf16().count();assert!(apply(&original,&patch(&original,3,end,"joined")).unwrap_err().contains("paragraph"));assert_eq!(original,scene());}
 #[test]fn retains_hard_breaks_and_supports_replacement_newlines_inside_one_block(){let mut original=scene();original.document["content"][0]["content"]=json!([{"type":"text","text":"A","marks":[{"type":"bold"}]},{"type":"hardBreak"},{"type":"text","text":"B"}]);let edited=apply(&original,&patch(&original,0,3,"X\nY")).unwrap();assert_eq!(plain_text(&edited.document),"X\nY\n\n中文");assert_eq!(edited.document["content"][0]["content"][1]["type"],"hardBreak");assert_eq!(edited.document["content"][0]["content"][2]["marks"][0]["type"],"bold");}
 #[test]fn deletion_is_a_normal_rich_document_change(){let original=scene();let edited=apply(&original,&patch(&original,3,7,"")).unwrap();assert_eq!(plain_text(&edited.document),"مِ  plain 😀\n\n中文");}
}
