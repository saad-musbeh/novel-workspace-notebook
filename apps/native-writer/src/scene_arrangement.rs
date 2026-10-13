//! A scene placement form with the same draft ownership as writing and chapters.
use super::*;
use novel_storage::SceneArrangementSnapshot;

pub(super) struct SceneArrangementPage {
    pub widget:gtk::Box,
    pub chapter:gtk::ComboBoxText,
    pub position:gtk::SpinButton,
    pub dirty:Cell<bool>,
    pub saving:Cell<bool>,
    pub composing:Cell<bool>,
    pub loaded:Cell<bool>,
    pub status:gtk::Label,
    project_id:String,
    scene_id:String,
    title:gtk::Label,
    controls:gtk::Box,
    snapshot:RefCell<Option<SceneArrangementSnapshot>>,
    updating:Cell<bool>,
}
impl SceneArrangementPage {
    fn new(project_id:String,scene_id:String)->Rc<Self>{
        let widget=gtk::Box::new(gtk::Orientation::Vertical,12);
        widget.set_margin_start(32);widget.set_margin_end(32);widget.set_margin_top(26);widget.set_margin_bottom(20);
        let heading=gtk::Label::new(Some("Arrange scene"));heading.set_xalign(0.);heading.style_context().add_class("page-title");widget.pack_start(&heading,false,false,0);
        let title=gtk::Label::new(None);title.set_xalign(0.);title.set_line_wrap(true);title.set_selectable(true);widget.pack_start(&title,false,false,0);
        let controls=gtk::Box::new(gtk::Orientation::Vertical,8);widget.pack_start(&controls,false,false,0);
        let chapter=gtk::ComboBoxText::new();let label=gtk::Label::new(Some("Target chapter"));label.set_xalign(0.);label.set_mnemonic_widget(Some(&chapter));controls.pack_start(&label,false,false,0);controls.pack_start(&chapter,false,false,0);
        let position=gtk::SpinButton::with_range(1.,1.,1.);position.set_numeric(true);position.set_update_policy(gtk::SpinButtonUpdatePolicy::IfValid);
        let label=gtk::Label::new(Some("Position in chapter"));label.set_xalign(0.);label.set_mnemonic_widget(Some(&position));controls.pack_start(&label,false,false,0);controls.pack_start(&position,false,false,0);
        let hint=gtk::Label::new(Some("1 places this scene first. Moving a scene keeps its writing and revision history."));hint.set_xalign(0.);hint.set_line_wrap(true);controls.pack_start(&hint,false,false,0);
        let status=gtk::Label::new(Some("Loading saved arrangement…"));status.set_xalign(0.);status.set_line_wrap(true);status.set_selectable(true);widget.pack_start(&status,false,false,0);
        let page=Rc::new(Self{widget,title,chapter,position,status,controls,project_id,scene_id,dirty:Cell::new(false),saving:Cell::new(false),composing:Cell::new(false),loaded:Cell::new(false),snapshot:RefCell::new(None),updating:Cell::new(false)});
        let weak=Rc::downgrade(&page);page.chapter.connect_changed(move|_|if let Some(p)=weak.upgrade(){if !p.updating.get(){p.update_range();p.changed();}});
        let weak=Rc::downgrade(&page);page.position.connect_changed(move|_|if let Some(p)=weak.upgrade(){p.changed();});
        let weak=Rc::downgrade(&page);page.position.connect_preedit_changed(move|_,text|if let Some(p)=weak.upgrade(){p.composing.set(!text.is_empty());});
        let row=gtk::Box::new(gtk::Orientation::Horizontal,8);page.controls.pack_start(&row,false,false,0);
        let weak=Rc::downgrade(&page);button(&row,"Save arrangement",move||if let Some(p)=weak.upgrade(){if let Some(owner)=desk::workspace_for_widget(p.widget.upcast_ref()){p.save(&owner,|_|{});}});
        let weak=Rc::downgrade(&page);button(&row,"Discard changes",move||if let Some(p)=weak.upgrade(){if !p.saving.get() && !p.composing.get(){p.discard();}});
        page.controls.set_sensitive(false);page
    }
    fn changed(&self){if !self.updating.get()&&self.loaded.get(){self.dirty.set(true);self.status.set_text("Unsaved arrangement · Save to keep it");}}
    fn update_range(&self){
        if let (Some(snapshot),Some(target))=(self.snapshot.borrow().as_ref(),self.chapter.active_id()){
            if let Some(chapter)=snapshot.chapters.iter().find(|c|c.chapter.id==target.as_str()){let count=chapter.scenes.iter().filter(|s|!s.deleted&&s.id!=self.scene_id).count();self.position.set_range(1.,(count+1) as f64);}
        }
    }
    fn restore(&self,snapshot:&SceneArrangementSnapshot){
        self.updating.set(true);self.title.set_text(&snapshot.title);self.chapter.remove_all();
        for c in &snapshot.chapters{self.chapter.append(Some(&c.chapter.id),&c.chapter.title);}
        self.chapter.set_active_id(Some(&snapshot.source_chapter_id));
        let source=snapshot.chapters.iter().find(|c|c.chapter.id==snapshot.source_chapter_id).unwrap();
        let visible=source.scenes.iter().filter(|s|!s.deleted).collect::<Vec<_>>();
        self.position.set_range(1.,visible.len() as f64);self.position.set_value((visible.iter().position(|s|s.id==self.scene_id).unwrap()+1) as f64);
        self.updating.set(false);self.dirty.set(false);
    }
    pub fn discard(&self){if let Some(snapshot)=self.snapshot.borrow().as_ref(){self.restore(snapshot);self.status.set_text("Arrangement changes discarded.");}}
    fn load(self:&Rc<Self>,owner:&Rc<Workspace>,done:impl FnOnce()+'static){
        let weak=Rc::downgrade(self);
        owner.client.rpc("scene_arrangement_snapshot",json!({"projectId":self.project_id,"sceneId":self.scene_id}),move|reply|{
            let Some(p)=weak.upgrade()else{done();return;};if desk::workspace_for_widget(p.widget.upcast_ref()).is_none(){done();return;}
            match reply.and_then(|v|serde_json::from_value::<SceneArrangementSnapshot>(v).map_err(|_|"Unsupported saved arrangement.".into())){
                Ok(snapshot)=>{p.restore(&snapshot);*p.snapshot.borrow_mut()=Some(snapshot);p.loaded.set(true);p.controls.set_sensitive(true);p.status.set_text("Saved locally · Manual saving");},
                Err(e)=>{p.loaded.set(false);p.controls.set_sensitive(false);p.status.set_text(&format!("Could not load arrangement. Reopen Arrange scene. {e}"));}
            }done();
        });
    }
    pub fn save(self:&Rc<Self>,owner:&Rc<Workspace>,callback:impl FnOnce(Result<(),String>)+'static){
        let result=(||{
            if self.saving.get(){return Err("The arrangement is still saving.".to_owned());}
            if self.composing.get(){return Err("Finish entering the arrangement before saving.".to_owned());}
            if !self.dirty.get(){return Ok(None);}
            if owner.desk.is_closed()||*owner.pid.borrow()!=self.project_id{return Err("The novel changed. Your arrangement stays in this page.".to_owned());}
            let snapshot=self.snapshot.borrow().clone().ok_or("Wait for the saved arrangement to load.")?;
            let target=self.chapter.active_id().ok_or("Select a target chapter.")?;
            let chapter=snapshot.chapters.iter().find(|c|c.chapter.id==target.as_str()).ok_or("Select a chapter in this novel.")?;
            let position=self.position.text().parse::<usize>().map_err(|_|"Enter a whole scene position.".to_owned())?;
            let max=chapter.scenes.iter().filter(|s|!s.deleted&&s.id!=self.scene_id).count()+1;
            if position==0||position>max{return Err("Scene position is outside this chapter's range.".to_owned());}
            Ok(Some(json!({"snapshot":snapshot,"targetChapterId":target.as_str(),"position":position-1})))
        })();
        let args=match result{Ok(Some(args))=>args,Ok(None)=>{callback(Ok(()));return;},Err(e)=>{self.status.set_text(&e);callback(Err(e));return;}};
        self.saving.set(true);self.controls.set_sensitive(false);self.status.set_text("Saving arrangement…");let p=self.clone();
        owner.client.rpc("arrange_scene",args,move|reply|match reply{
            Ok(_)=>{p.dirty.set(false);p.loaded.set(false);if let Some(a)=desk::workspace_for_widget(p.widget.upcast_ref()){a.load_index();let finished=p.clone();p.load(&a,move||{finished.saving.set(false);callback(Ok(()));});}else{p.saving.set(false);callback(Ok(()));}},
            Err(e)=>{p.saving.set(false);p.controls.set_sensitive(true);p.status.set_text(&format!("Arrangement not saved; changes retained. {e}"));callback(Err(e));}
        });
    }
}
impl Workspace {
    pub(super) fn arrange_scene(self:&Rc<Self>){
        let a=desk::effective(self);let scene=a.selected().and_then(|(id,kind)|(kind=="scene").then_some(id));
        let Some(scene)=scene else{a.error("Select a scene to arrange first.");return;};let out=a.clone();a.navigate(move||{
            let page=SceneArrangementPage::new(out.pid.borrow().clone(),scene);out.scene_arrangement_pages.borrow_mut().push(page.clone());out.install(&page.widget,"Arrange scene",true);page.load(&out,||{});
        });
    }
}
