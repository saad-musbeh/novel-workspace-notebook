//! Validated native shortcut preferences, additive to existing project settings.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeShortcutBinding {
    pub code:String,
    pub ctrl:bool,
    pub alt:bool,
    pub shift:bool,
    pub meta:bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct NativeShortcutSettings {
    pub schema_version:u32,
    pub bindings:BTreeMap<String,Vec<NativeShortcutBinding>>,
}
pub const NATIVE_SHORTCUT_ACTIONS:&[(&str,&str)]=&[
    ("workspace.save","Save tab"),("workspace.newTab","New tab"),("workspace.closeTab","Close tab"),("workspace.find","Find in scene"),
    ("workspace.nextTab","Next tab"),("workspace.previousTab","Previous tab"),("workspace.detachTab","Move tab to new window"),
    ("workspace.back","Back to previous place"),("workspace.forward","Forward to next place"),("workspace.fullscreen","Toggle fullscreen"),
    ("editor.bold","Bold"),("editor.italic","Italic"),("editor.underline","Underline"),("editor.selectAll","Select all"),
    ("editor.undo","Undo"),("editor.redo","Redo"),("editor.copy","Copy"),("editor.cut","Cut"),("editor.paste","Paste"),("editor.unicodeToggle","Convert Unicode code / character"),
    ("editor.sequence1","Run command sequence 1"),("editor.sequence2","Run command sequence 2"),
    ("editor.sequence3","Run command sequence 3"),("editor.sequence4","Run command sequence 4"),
    ("editor.sequence5","Run command sequence 5"),("editor.sequence6","Run command sequence 6"),
    ("editor.sequence7","Run command sequence 7"),("editor.sequence8","Run command sequence 8"),
];
const ADDITIVE_ACTIONS:&[&str]=&["workspace.back","workspace.forward","workspace.fullscreen","editor.sequence1","editor.sequence2","editor.sequence3","editor.sequence4","editor.sequence5","editor.sequence6","editor.sequence7","editor.sequence8"];
fn binding(code:&str,ctrl:bool,shift:bool,alt:bool)->NativeShortcutBinding{NativeShortcutBinding{code:code.into(),ctrl,alt,shift,meta:false}}
impl Default for NativeShortcutSettings {
    fn default()->Self{
        let defaults=[("workspace.save","KeyS",true,false,false),("workspace.newTab","KeyT",true,false,false),("workspace.closeTab","KeyW",true,false,false),("workspace.find","KeyF",true,false,false),("workspace.nextTab","Tab",true,false,false),("workspace.previousTab","Tab",true,true,false),("workspace.detachTab","KeyN",true,true,false),("editor.bold","KeyB",true,false,false),("editor.italic","KeyI",true,false,false),("editor.underline","KeyU",true,false,false),("editor.selectAll","KeyA",true,false,false),("editor.undo","KeyZ",true,false,false),("editor.redo","KeyY",true,false,false),("editor.copy","KeyC",true,false,false),("editor.cut","KeyX",true,false,false),("editor.paste","KeyV",true,false,false),("editor.unicodeToggle","KeyX",false,false,true)];
        let mut bindings:BTreeMap<String,Vec<NativeShortcutBinding>>=defaults.into_iter().map(|(id,key,c,s,a)|(id.into(),vec![binding(key,c,s,a)])).collect();
        bindings.insert("workspace.back".into(),vec![binding("ArrowLeft",false,false,true)]);
        bindings.insert("workspace.forward".into(),vec![binding("ArrowRight",false,false,true)]);
        bindings.insert("workspace.fullscreen".into(),vec![binding("F11",false,false,false)]);
        for slot in 1..=8{bindings.insert(format!("editor.sequence{slot}"),vec![]);}
        bindings.get_mut("editor.redo").unwrap().push(binding("KeyZ",true,true,false));Self{schema_version:1,bindings}
    }
}
impl NativeShortcutBinding {
    pub fn validate(&self)->Result<(),String>{
        if self.meta{return Err("Meta shortcuts are reserved for the desktop.".into())}
        if self.ctrl&&self.alt{return Err("Ctrl+Alt combinations are reserved for AltGr text input.".into())}
        let letter=self.code.strip_prefix("Key").is_some_and(|s|s.len()==1&&s.as_bytes()[0].is_ascii_uppercase());
        let digit=self.code.strip_prefix("Digit").is_some_and(|s|s.len()==1&&s.as_bytes()[0].is_ascii_digit());
        let function=self.code.strip_prefix('F').and_then(|s|s.parse::<u8>().ok()).filter(|n|(1..=24).contains(n)&&self.code==format!("F{n}"));
        if letter||digit{
            if !self.ctrl&&!self.alt{return Err("Letters and numbers require Ctrl or Alt; ordinary typing is reserved.".into())}
        }else if self.code=="Tab"{
            if !self.ctrl||self.alt{return Err("Only Ctrl+Tab combinations can be assigned; normal Tab input and desktop switching are reserved.".into())}
        }else if matches!(self.code.as_str(),"ArrowLeft"|"ArrowRight"){
            if !self.alt||self.ctrl{return Err("Arrow shortcuts require Alt; ordinary, Shift and Ctrl navigation remain reserved for editing.".into())}
        }else if let Some(number)=function{
            if [1,6,7].contains(&number)||(self.alt&&number==4)||(number==11&&(self.ctrl||self.alt||self.shift)){return Err("This function key is reserved for help, peek, fullscreen or desktop controls.".into())}
        }else{return Err("Use a letter, number, Ctrl+Tab, Alt+ArrowLeft/Right or an available function key.".into())}
        Ok(())
    }
    pub fn display(&self)->String{
        let key=self.code.strip_prefix("Key").or_else(||self.code.strip_prefix("Digit")).unwrap_or(&self.code);
        format!("{}{}{}{}",if self.ctrl{"Ctrl+"}else{""},if self.alt{"Alt+"}else{""},if self.shift{"Shift+"}else{""},key)
    }
}
impl NativeShortcutSettings {
    pub fn validate(&self)->Result<(),String>{
        if self.schema_version!=1{return Err("Unsupported native shortcuts version.".into())}
        if self.bindings.keys().any(|id|!NATIVE_SHORTCUT_ACTIONS.iter().any(|(known,_)|id.as_str()==*known))||NATIVE_SHORTCUT_ACTIONS.iter().any(|(id,_)|!ADDITIVE_ACTIONS.contains(id)&&!self.bindings.contains_key(*id)){return Err("Unsupported or missing native shortcut actions.".into())}
        let mut used=BTreeSet::new();
        for (action,bindings) in &self.bindings{
            if bindings.len()>3{return Err("An action can have at most three shortcuts.".into())}
            for binding in bindings{
                binding.validate()?;
                if binding.code=="F11"&&action!="workspace.fullscreen"{return Err("F11 is reserved for the fullscreen action.".into())}
                if action.starts_with("workspace.")&&binding.ctrl&&!binding.alt&&["KeyA","KeyC","KeyX","KeyV","KeyZ","KeyY"].contains(&binding.code.as_str()){
                    return Err("Workspace shortcuts cannot replace standard text-field selection, clipboard or Undo keys.".into())
                }
                if !used.insert(binding){return Err(format!("{} is assigned more than once ({action}).",binding.display()))}
            }
        }Ok(())
    }
    pub fn from_project_settings(value:&Value)->Result<Self,String>{
        let Some(shortcuts)=value.get("nativeShortcuts")else{return Ok(Self::default())};
        let mut parsed:Self=serde_json::from_value(shortcuts.clone()).map_err(|_|"Invalid native shortcut settings fields.")?;parsed.validate()?;
        // Additive schema-one defaults yield to explicit existing assignments.
        // An explicitly cleared new action stays cleared. This changes only the
        // read projection; stored preferences wait for the author's manual Save.
        let defaults=Self::default();let occupied=parsed.bindings.values().flatten().cloned().collect::<BTreeSet<_>>();
        for id in ADDITIVE_ACTIONS{if !parsed.bindings.contains_key(*id){parsed.bindings.insert((*id).into(),defaults.bindings[*id].iter().filter(|binding|!occupied.contains(*binding)).cloned().collect());}}
        parsed.validate()?;Ok(parsed)
    }
    pub fn action(&self,scope:&str,binding:&NativeShortcutBinding)->Option<&str>{
        self.bindings.iter().find(|(id,bindings)|id.starts_with(scope)&&bindings.contains(binding)).map(|(id,_)|id.as_str())
    }
}
/// Human-readable form input, e.g. `Ctrl+Shift+S, F8`; blank disables an action.
pub fn parse_native_shortcut_list(text:&str)->Result<Vec<NativeShortcutBinding>,String>{
    if text.trim().is_empty(){return Ok(vec![])}
    let entries=text.split(',').collect::<Vec<_>>();if entries.len()>3{return Err("An action can have at most three shortcuts.".into())}
    entries.into_iter().map(|entry|{
        let parts=entry.trim().split('+').map(str::trim).collect::<Vec<_>>();
        let mut chord=binding("",false,false,false);let mut seen=BTreeSet::new();
        for modifier in &parts[..parts.len().saturating_sub(1)]{
            let normalized=modifier.to_ascii_lowercase();if !seen.insert(normalized.clone()){return Err("A shortcut repeats a modifier.".into())}
            match normalized.as_str(){"ctrl"|"control"=>{if chord.ctrl{return Err("A shortcut repeats Ctrl.".into())}chord.ctrl=true},"alt"=>chord.alt=true,"shift"=>chord.shift=true,"meta"|"super"=>chord.meta=true,_=>return Err("Use Ctrl, Alt or Shift before the key.".into())}
        }
        let key=parts.last().unwrap_or(&"").to_ascii_uppercase();chord.code=if key.len()==1&&key.as_bytes()[0].is_ascii_alphabetic(){format!("Key{key}")}else if key.len()==1&&key.as_bytes()[0].is_ascii_digit(){format!("Digit{key}")}else if key=="TAB"{"Tab".into()}else if key=="LEFT"||key=="ARROWLEFT"{"ArrowLeft".into()}else if key=="RIGHT"||key=="ARROWRIGHT"{"ArrowRight".into()}else{key};chord.validate()?;Ok(chord)
    }).collect()
}
