//! Atomic JSON checkpoints, compatible with the former Python helper's schema.
use crate::{
    engine::NativeEditor, model::{
        self, Result, State, TextModel, HISTORY_BYTES
    }
};
use serde_json::{
    json, Value
};
use std::{
    collections::{
        BTreeSet, HashMap
    }, fs::{
        self, File, OpenOptions
    }, io::{
        Read, Write
    }, path::Path, sync::Arc
};
pub const MAX_BYTES: usize = 128 * 1024 * 1024;
fn encode_state(state: &State, blocks: &mut Vec<Value>, identities: &mut HashMap<usize,usize>) -> Value {
    let refs: Vec<_>=state.blocks.iter().map(|block| *identities.entry(Arc::as_ptr(block) as usize).or_insert_with(||{
        blocks.push(block.as_ref().clone());
        blocks.len()-1
    })).collect();
    json!([refs,state.cursor,state.anchor,state.typing_marks.as_ref().map(|m|m.iter().collect::<Vec<_>>())])
}
pub fn write(directory: &Path, editor: &NativeEditor) -> Result<()> {
    let model=&editor.model;
    let mut blocks=Vec::new();
    let mut identities=HashMap::new();
    let state=encode_state(&model.state,&mut blocks,&mut identities);
    let undo: Vec<_>=model.undo.iter().map(|(s,c)|json!([encode_state(s,&mut blocks,&mut identities),c])).collect();
    let redo: Vec<_>=model.redo.iter().map(|(s,c)|json!([encode_state(s,&mut blocks,&mut identities),c])).collect();
    let payload=json!({
        "schemaVersion":1,"opening":editor.opening,"original":model.original,"state":state,"undo":undo,"redo":redo,"blocks":blocks,"trimmed":model.undo_trimmed,"version":model.version,"top":editor.top,"height":editor.height,"width":editor.layout.width,"document":model.document()
    });
    let data=serde_json::to_vec(&payload).map_err(|e|e.to_string())?;
    if data.len()>MAX_BYTES{
        return Err("This scene’s Undo checkpoint is too large. The open editor is retained.".into());
    }
    let temporary=directory.join("checkpoint.pending");
    let mut options=OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file=options.open(&temporary).map_err(|e|e.to_string())?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|e|e.to_string())?;
    }
    file.write_all(&data).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;
    drop(file);
    fs::rename(&temporary,directory.join("checkpoint.json")).map_err(|e|e.to_string())?;
    File::open(directory).and_then(|f|f.sync_all()).map_err(|e|e.to_string())?;
    Ok(())
}
fn state(raw: &Value, blocks: &[Arc<Value>], lengths: &[usize]) -> Result<State> {
    let array=raw.as_array().filter(|v|v.len()==4).ok_or("Invalid private editor state.")?;
    let refs=array[0].as_array().filter(|v|!v.is_empty() && v.len()<=20_000).ok_or("Invalid private paragraph reference.")?;
    let indices: Vec<_>=refs.iter().map(|v|v.as_u64().map(|n|n as usize).filter(|n|*n<blocks.len()).ok_or("Invalid private paragraph reference.")).collect::<std::result::Result<_,_>>()?;
    let position=|raw:&Value| -> Result<[usize;
    2]> {
        let p=raw.as_array().filter(|v|v.len()==2).ok_or("Invalid private cursor position.")?;
        let block=p[0].as_u64().filter(|i|*i<(indices.len() as u64)).ok_or("Invalid private cursor position.")? as usize;
        let offset=p[1].as_u64().filter(|i|*i<=lengths[indices[block]] as u64).ok_or("Invalid private cursor position.")? as usize;
        Ok([block,offset])
    };
    let marks=if array[3].is_null(){
        None
    }else{
        let marks=array[3].as_array().ok_or("Invalid private typing marks.")?;
        Some(marks.iter().map(|m|m.as_str().filter(|s|model::MARKS.contains(s)).map(str::to_owned).ok_or("Invalid private typing marks.")).collect::<std::result::Result<BTreeSet<_>,_>>()?)
    };
    let cursor=position(&array[1])?;
    let anchor=position(&array[2])?;
    Ok(State {
        blocks:indices.into_iter().map(|i|blocks[i].clone()).collect(),cursor,anchor,typing_marks:marks
    })
}
pub fn read(directory: &Path) -> Result<(Value,TextModel)> {
    let mut data=Vec::new();
    File::open(directory.join("checkpoint.json")).and_then(|f|f.take(MAX_BYTES as u64+1).read_to_end(&mut data)).map_err(|e|e.to_string())?;
    if data.len()>MAX_BYTES{
        return Err("The private editor checkpoint exceeds its limit.".into());
    }
    let value: Value=serde_json::from_slice(&data).map_err(|_|"Invalid private editor checkpoint.")?;
    if value["schemaVersion"]!=1 || value["opening"]["type"]!="open"{
        return Err("Unsupported private editor checkpoint.".into());
    }
    model::validate_document(&value["original"])?;
    model::validate_document(&value["document"])?;
    let nodes=value["blocks"].as_array().filter(|v|v.len()<=3_000_000).ok_or("Invalid private paragraph table.")?;
    let mut blocks=Vec::new();
    let mut lengths=Vec::new();
    for block in nodes {
        model::validate_document(&json!({
            "type":"doc","content":[block]
        }))?;
        lengths.push(model::len(&model::text_of(block)));
        blocks.push(Arc::new(block.clone()));
    }
    let history=|raw:&Value| -> Result<Vec<(State,usize)>> {
        raw.as_array().filter(|v|v.len()<=512).ok_or("Invalid private Undo history.")?.iter().map(|entry| {
            let entry=entry.as_array().filter(|v|v.len()==2).ok_or("Invalid private Undo entry.")?;
            let cost=entry[1].as_u64().filter(|c|*c<=HISTORY_BYTES as u64).ok_or("Invalid private Undo accounting.")? as usize;
            Ok((state(&entry[0],&blocks,&lengths)?,cost))
        }).collect()
    };
    let mut restored=TextModel::new(value["original"].clone())?;
    restored.state=state(&value["state"],&blocks,&lengths)?;
    restored.undo=history(&value["undo"])?;
    restored.redo=history(&value["redo"])?;
    restored.undo_bytes=restored.undo.iter().map(|(_,c)|c).sum();
    if restored.undo_bytes>HISTORY_BYTES{
        return Err("Invalid private Undo accounting.".into());
    }
    restored.undo_trimmed=value["trimmed"].as_bool().ok_or("Invalid private Undo accounting.")?;
    restored.version=value["version"].as_u64().ok_or("Invalid private Undo accounting.")?;
    if restored.document()!=value["document"]{
        return Err("The private editor checkpoint does not match its document.".into());
    }
    if !value["top"].as_u64().is_some_and(|n|n<=100_000_000) || !value["height"].as_u64().is_some_and(|n|(120..=2160).contains(&n)) || !value["width"].as_u64().is_some_and(|n|(240..=4096).contains(&n)){
        return Err("Invalid private editor viewport.".into());
    }
    Ok((value,restored))
}
