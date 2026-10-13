//! Saved Part headings organize canonical chapters without moving manuscript text.
use super::*;
use novel_storage::{ManuscriptParts,ManuscriptPartsSnapshot};
impl Workspace {
    pub(super) fn refresh_parts_outline(self:&Rc<Self>) {
        let epoch=self.parts_outline_epoch.get().wrapping_add(1);self.parts_outline_epoch.set(epoch);
        self.parts_outline.borrow_mut().take();self.populate_outline();self.populate_book();
        let pid=self.pid.borrow().clone();if pid.is_empty(){return;}
        let a=Rc::downgrade(self);
        self.client.rpc("parts_edit_snapshot",json!({"projectId":pid}),move|reply|{
            let Some(a)=a.upgrade()else{return;};
            if a.desk.is_closed()||*a.pid.borrow()!=pid||a.parts_outline_epoch.get()!=epoch{return;}
            let checked=reply.and_then(|v|serde_json::from_value::<ManuscriptPartsSnapshot>(v).map_err(|e|e.to_string())).and_then(|s|{
                let index=a.index.borrow();
                if s.project_id!=pid||serde_json::to_value(&s.chapters).map_err(|e|e.to_string())?!=index["chapters"]||s.chapter_number_start!=index["chapterNumberStart"].as_i64().unwrap_or(1){return Err("Canonical chapters changed while Part headings were read. Refresh to check them again.".into());}
                s.planning.as_ref().map(|p|ManuscriptParts::from_planning(&p.payload)).transpose().map(|p|p.unwrap_or_default())
            });
            match checked {
                Ok(parts)=>{*a.parts_outline.borrow_mut()=Some(parts);a.tree.set_tooltip_text(None);a.populate_outline();a.populate_book();}
                Err(e)=>{a.tree.set_tooltip_text(Some(&format!("Saved Part headings unavailable; canonical chapters remain visible. {e}")));}
            }
        });
    }
    pub(super) fn chapter_part(&self,chapter:&str)->Option<(String,String)> {
        let parts=self.parts_outline.borrow();let parts=parts.as_ref()?;
        let id=&parts.chapters.iter().find(|p|p.chapter_id==chapter)?.part_id;
        parts::chapter_heading(parts,chapter).map(|title|(id.clone(),title))
    }
}
