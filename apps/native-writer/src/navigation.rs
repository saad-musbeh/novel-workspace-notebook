//! Window-local reading locations, captured after manual draft decisions.
use super::*;
use sha2::{Digest,Sha256};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TrailEntry {
    pub scene_id: String,
    location: Option<Value>,
    revision_id:Option<String>,
    top: u64,
}
impl TrailEntry {
    fn new(scene_id: &str) -> Self {
        Self { scene_id: scene_id.into(), location: None, revision_id:None,top: 0 }
    }
}
fn push_trail(trail:&mut Vec<TrailEntry>,index:&mut usize,scene:&str){
    if trail.get(*index).is_none_or(|entry|entry.scene_id!=scene){
        trail.truncate(index.saturating_add(1));trail.push(TrailEntry::new(scene));
        if trail.len()>256{trail.remove(0);}*index=trail.len()-1;
    }
}
fn frame_location(frame:&Value)->Result<(Value,u64),String>{
    let document=frame.get("document").filter(|d|d["type"]=="doc").ok_or("The last confirmed writing frame has no document.")?;
    let blocks=document["content"].as_array().ok_or("The last confirmed writing frame has no paragraphs.")?;
    let coordinate=|key:&str|->Result<Value,String>{
        let value=&frame["position"][key];let position=value.as_array().filter(|p|p.len()==2).ok_or("The last confirmed writing frame has no valid selection.")?;
        let block=position[0].as_u64().and_then(|p|usize::try_from(p).ok()).filter(|p|*p<blocks.len()).ok_or("The confirmed paragraph position is outside this scene.")?;
        let length=novel_storage::plain_text(&blocks[block]).chars().count();
        if !position[1].as_u64().is_some_and(|p|p<=length as u64){return Err("The confirmed text position is outside this paragraph.".into())}Ok(value.clone())
    };
    let cursor=coordinate("cursor")?;let anchor=coordinate("anchor")?;let top=frame["top"].as_u64().ok_or("The last confirmed frame has no scroll position.")?;
    let bytes=serde_json::to_vec(document).map_err(|_|"The confirmed document could not be captured.")?;
    Ok((json!({"documentHash":format!("{:x}",Sha256::digest(bytes)),"cursor":cursor,"anchor":anchor}),top))
}
impl Workspace {
    pub(super) fn record_navigation(&self, scene: &str) {
        let mut trail = self.trail.borrow_mut();
        let mut index=self.trail_index.get();push_trail(&mut trail,&mut index,scene);self.trail_index.set(index);
    }
    pub(super) fn capture_departure(self: &Rc<Self>, done: impl FnOnce(bool) + 'static) {
        let pane = self.active.borrow().clone().filter(|p| self.current_widget() == Some(p.widget.clone().upcast()));
        let Some(pane) = pane else { done(true); return; };
        if pane.composing.get() || pane.pending.get() != 0 || pane.saving.get() || !pane.open.get() {
            self.error("The writing page is still busy. Its reading location was not captured.");
            done(false); return;
        }
        let captured=frame_location(&pane.frame.borrow());
        match captured{
            Ok((location,top))=>{
                let revision=pane.parent.borrow().clone();
                if let Some(entry)=self.trail.borrow_mut().get_mut(self.trail_index.get()).filter(|entry|entry.scene_id==pane.scene_id){entry.location=Some(location);entry.top=top;entry.revision_id=Some(revision);}
                done(true);
            },
            Err(error)=>{self.error(&format!("Reading location was not captured. {error}"));done(false);}
        }
    }
    pub(super) fn back(self: &Rc<Self>) {
        if let Some(index) = self.trail_index.get().checked_sub(1) { self.visit_trail(index); }
    }
    pub(super) fn forward(self: &Rc<Self>) {
        let index = self.trail_index.get() + 1;
        if index < self.trail.borrow().len() { self.visit_trail(index); }
    }
    fn visit_trail(self: &Rc<Self>, index: usize) {
        let Some(entry) = self.trail.borrow().get(index).cloned() else { return; };
        if entry.location.is_none() { self.error("This place has no captured reading location."); return; }
        let a = self.clone();
        self.navigate(move || {
            // Generic navigation releases its decision lock before running an
            // action. Keep this action locked until its exact restore completes.
            a.desk.busy.set(true);a.window.set_sensitive(false);
            let generation = a.desk.generation.get(); let out = a.clone(); let id = entry.scene_id.clone();
            a.ensure_pane(&id, move |reply| match reply {
                Ok(pane) => out.restore_trail(pane, index, entry, generation, std::time::Instant::now()),
                Err(error) => {out.error(&error);out.finish_trail(generation);},
            });
        });
    }
    fn restore_trail(self: &Rc<Self>, pane: Rc<EditorPane>, index: usize, entry: TrailEntry, generation: u64, started: std::time::Instant) {
        if self.desk.is_closed() || self.desk.generation.get() != generation || self.trail.borrow().get(index) != Some(&entry) {self.finish_trail(generation); return; }
        if !pane.open.get() || pane.pending.get() != 0 || pane.saving.get() {
            if started.elapsed() > Duration::from_secs(15) { self.error("The scene is still busy. The current page stays open.");self.finish_trail(generation); return; }
            let a = self.clone();
            glib::timeout_add_local_once(Duration::from_millis(30), move || a.restore_trail(pane, index, entry, generation, started)); return;
        }
        if pane.composing.get() { self.error("Finish the current input composition before returning to this place.");self.finish_trail(generation); return; }
        let a=self.clone();self.client.rpc("load_project_scene",json!({"projectId":self.pid.borrow().clone(),"sceneId":entry.scene_id}),move|reply|{
            if a.desk.is_closed()||a.desk.generation.get()!=generation||a.trail.borrow().get(index)!=Some(&entry){a.finish_trail(generation);return;}
            match reply{
                Ok(saved) if saved["revisionId"].as_str()==entry.revision_id.as_deref()&&saved["revisionId"]==*pane.parent.borrow()=>a.restore_checked_trail(pane,index,entry,generation),
                Ok(_)=>{a.error("The saved scene changed since this reading location. The current page and captured offsets are preserved.");a.finish_trail(generation);},
                Err(error)=>{a.error(&format!("Could not return to the captured place. The current page stays open. {error}"));a.finish_trail(generation);}
            }
        });
    }
    fn restore_checked_trail(self:&Rc<Self>,pane:Rc<EditorPane>,index:usize,entry:TrailEntry,generation:u64){
        if pane.composing.get()||pane.pending.get()!=0||pane.saving.get(){self.error("The scene became busy before its reading location could be restored.");self.finish_trail(generation);return;}
        let a = self.clone(); let p = pane.clone();
        pane.request(json!({"type":"command","command":"restoreNavigation","bookmark":entry.location,"top":entry.top,"followCursor":false}), false, move |reply| {
            if a.desk.is_closed() || a.desk.generation.get() != generation || a.trail.borrow().get(index) != Some(&entry) {a.finish_trail(generation); return; }
            match reply {
                Ok(_) => { a.trail_index.set(index); a.show_pane(p, false); }
                Err(error) => a.error(&format!("Could not return to the captured place. The current page stays open. {error}")),
            }
            a.finish_trail(generation);
        });
    }
    fn finish_trail(&self,generation:u64){if self.desk.generation.get()==generation&&!self.desk.is_closed(){self.desk.busy.set(false);self.window.set_sensitive(true);}}
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn acknowledged_frame_capture_uses_exact_rich_unicode_json_and_scalar_positions(){
        let document=json!({"type":"doc","content":[{"type":"paragraph","attrs":{"dir":"rtl","lang":"ar"},"content":[{"type":"text","text":"مِ 中文 𠀀 👩🏽‍🚀","marks":[{"type":"bold"}]},{"type":"hardBreak"},{"type":"text","text":"e\u{301}"}]}]});
        let frame=json!({"document":document,"position":{"cursor":[0,8],"anchor":[0,2]},"top":320});let (location,top)=frame_location(&frame).unwrap();assert_eq!(top,320);assert_eq!(location["cursor"],json!([0,8]));assert_eq!(location["anchor"],json!([0,2]));assert_eq!(location["documentHash"],format!("{:x}",Sha256::digest(serde_json::to_vec(&document).unwrap())));
        let mut invalid=frame;invalid["position"]["cursor"]=json!([0,1000]);assert!(frame_location(&invalid).is_err());invalid["position"]["cursor"]=json!([1,0]);assert!(frame_location(&invalid).is_err());invalid["position"]["cursor"]=json!([0,0]);invalid["top"]=json!(-1);assert!(frame_location(&invalid).is_err());
    }
    #[test]fn trail_is_bounded_and_new_branches_replace_only_forward_entries(){
        let mut trail=vec![];let mut index=0;for i in 0..300{push_trail(&mut trail,&mut index,&format!("scene-{i}"));}assert_eq!(trail.len(),256);assert_eq!(index,255);assert_eq!(trail[0].scene_id,"scene-44");
        index=10;let scene=trail[index].scene_id.clone();push_trail(&mut trail,&mut index,&scene);assert_eq!(trail.len(),256);assert_eq!(index,10);push_trail(&mut trail,&mut index,"branch");assert_eq!(trail.len(),12);assert_eq!(index,11);assert_eq!(trail.last().unwrap().scene_id,"branch");assert_eq!(trail[10].scene_id,scene);
    }
}
